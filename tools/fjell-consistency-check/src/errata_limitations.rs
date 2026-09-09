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
use fjell_consistency_check::errata::{is_accepted, parse_summary_rows};
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
///
/// RFC-0.29-002 R3/D1: this used to be `!limitations_src.contains(id)` — a
/// bare substring search that anything in the file could satisfy, not
/// only a genuine disclosure entry. Every real entry in
/// `v1-limitations.md` writes the id in the file's own established
/// convention, `**E-NNN**` (bold), so an id appearing only as an
/// incidental, unformatted mention elsewhere (a cross-reference in
/// unrelated prose, a quoted log line) would previously satisfy `contains`
/// without the erratum actually being disclosed there. Checking for the
/// bold form specifically is structural, not a wider literal: it is the
/// one shape every passing entry already has.
pub fn run_check(errata_src: &str, limitations_src: &str) -> ExitCode {
    let accepted = parse_accepted_errata(errata_src);
    let missing: Vec<&String> = accepted
        .iter()
        .filter(|id| !limitations_src.contains(&format!("**{id}**")))
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

/// The `## Summary` table's rows whose status cell is `ACCEPTED`
/// (annotation or not — RFC-0.29-002 D1), via the one shared parser.
fn parse_accepted_errata(src: &str) -> Vec<String> {
    parse_summary_rows(src)
        .into_iter()
        .filter(|row| is_accepted(&row.status))
        .map(|row| row.id)
        .collect()
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

    /// RFC-0.29-002 R3 required demonstration: the input the old bare
    /// `.contains(id)` check missed. `E-011` appears here only as an
    /// unformatted, incidental mention (a commit-message-style aside) —
    /// not the file's own established `**E-011**` disclosure convention —
    /// so this is not a real disclosure entry and must fail.
    #[test]
    fn bare_unformatted_mention_is_not_a_real_disclosure() {
        let limitations =
            "Errata **E-004** (ACCEPTED); see the diff that mentioned E-011 in passing";
        assert!(limitations.contains("E-011"), "fixture sanity check");
        assert_eq!(
            run_check(ERRATA_FIXTURE, limitations),
            ExitCode::FAILURE,
            "an unformatted, non-bold mention of an id must not count as disclosure"
        );
    }
}
