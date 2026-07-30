//! Slice 4 (RFC-v0.22-001): the `errata-limitations` subcheck.
//!
//! Every erratum in `docs/rfcs/ERRATA.md` whose `## Summary` table marks it
//! `ACCEPTED` is, by definition, a disclosed v1.0 limitation. Gate 9
//! (`docs/release/v1-limitations.md`) is documented as "the single
//! authoritative list" for that release gate — an ACCEPTED erratum missing
//! from it is exactly the recorded E-011 instance (RFC-v0.22-001
//! Motivation #2): the register said ACCEPTED, the limitations doc did not
//! carry it, and Gate 7 (0 OPEN) reported green regardless.

use crate::read_file;
use std::process::ExitCode;

const ERRATA_PATH: &str = "docs/rfcs/ERRATA.md";
const LIMITATIONS_PATH: &str = "docs/release/v1-limitations.md";

pub fn check() -> ExitCode {
    let Some(errata_src) = read_file(ERRATA_PATH) else {
        return ExitCode::FAILURE;
    };
    let Some(limitations_src) = read_file(LIMITATIONS_PATH) else {
        return ExitCode::FAILURE;
    };
    run_check(&errata_src, &limitations_src)
}

/// Core comparison, pure in its inputs for testing with synthetic fixtures.
pub fn run_check(errata_src: &str, limitations_src: &str) -> ExitCode {
    let accepted = parse_accepted_errata(errata_src);
    let missing: Vec<&String> = accepted
        .iter()
        .filter(|id| !limitations_src.contains(id.as_str()))
        .collect();

    if missing.is_empty() {
        println!(
            "errata-limitations: PASS ({} ACCEPTED errata, all referenced in {LIMITATIONS_PATH})",
            accepted.len()
        );
        ExitCode::SUCCESS
    } else {
        eprintln!("errata-limitations: FAIL");
        for m in &missing {
            eprintln!(
                "  {m} is ACCEPTED in {ERRATA_PATH} but not referenced in {LIMITATIONS_PATH}"
            );
        }
        ExitCode::FAILURE
    }
}

/// Parse the `## Summary` table's rows (`| E-XXX label | tracking | STATUS |`)
/// and return the IDs whose status cell starts with `ACCEPTED`.
fn parse_accepted_errata(src: &str) -> Vec<String> {
    let mut in_summary = false;
    let mut ids = Vec::new();
    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed == "## Summary" {
            in_summary = true;
            continue;
        }
        if !in_summary || !trimmed.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = trimmed
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        if cells.len() < 3 {
            continue;
        }
        if cells[0].eq_ignore_ascii_case("errata") || cells[0].starts_with("--") {
            continue; // header or separator row
        }
        let Some(id) = extract_erratum_id(cells[0]) else {
            continue;
        };
        if cells[2].starts_with("ACCEPTED") {
            ids.push(id);
        }
    }
    ids
}

/// Extract a leading `E-NNN` token from a summary-table first cell such as
/// `"E-004 hardware boot"`.
fn extract_erratum_id(cell: &str) -> Option<String> {
    let cell = cell.trim();
    let rest = cell.strip_prefix("E-")?;
    let digit_len = rest.chars().take_while(char::is_ascii_digit).count();
    if digit_len == 0 {
        return None;
    }
    Some(format!("E-{}", &rest[..digit_len]))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ERRATA_FIXTURE: &str = r#"
## Summary

| Errata | Tracking RFC | Status |
|--------|--------------|--------|
| E-001 closed thing | v0.16-001 | CLOSED |
| E-004 hardware boot | v0.16-005 | ACCEPTED (v1.0 limitation) |
| E-011 cap_install rights validation | v0.21.3-001 | ACCEPTED |
"#;

    #[test]
    fn parse_accepted_errata_finds_only_accepted_ids() {
        let ids = parse_accepted_errata(ERRATA_FIXTURE);
        assert_eq!(ids, vec!["E-004".to_string(), "E-011".to_string()]);
    }

    #[test]
    fn passes_when_every_accepted_erratum_is_referenced() {
        let limitations = "Errata **E-004** (ACCEPTED); Errata **E-011** (ACCEPTED)";
        assert_eq!(run_check(ERRATA_FIXTURE, limitations), ExitCode::SUCCESS);
    }

    /// Required failure demonstration: an ACCEPTED erratum absent from the
    /// limitations doc — the recorded E-011 instance.
    #[test]
    fn fails_when_an_accepted_erratum_is_missing_from_limitations() {
        // Limitations doc only mentions E-004; E-011 (ACCEPTED) is absent.
        let limitations = "Errata **E-004** (ACCEPTED)";
        assert_eq!(
            run_check(ERRATA_FIXTURE, limitations),
            ExitCode::FAILURE,
            "an ACCEPTED erratum missing from v1-limitations.md must fail the check"
        );
    }

    #[test]
    fn closed_errata_are_not_required_to_appear() {
        // E-001 is CLOSED, not ACCEPTED — its absence must not fail the check.
        let limitations = "Errata **E-004** (ACCEPTED); Errata **E-011** (ACCEPTED)";
        assert!(!limitations.contains("E-001"));
        assert_eq!(run_check(ERRATA_FIXTURE, limitations), ExitCode::SUCCESS);
    }

    #[test]
    fn extract_erratum_id_stops_at_first_non_digit() {
        assert_eq!(
            extract_erratum_id("E-004 hardware boot"),
            Some("E-004".to_string())
        );
        assert_eq!(extract_erratum_id("not an id"), None);
    }
}
