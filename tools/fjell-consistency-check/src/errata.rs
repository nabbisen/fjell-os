//! RFC-0.29-002 §7: the one shared parser for `docs/rfcs/ERRATA.md`'s
//! `## Summary` table.
//!
//! At least four instruments used to re-parse this table independently —
//! this crate's own `errata-tracking` and `errata-limitations` subchecks,
//! `release_rehearsal`'s Gate 7 (`crates/fjell-tools`, a different crate
//! entirely), and the register's own hand-maintained totals row — each
//! with its own idea of what a row is. Gate 7's copy was the worst of the
//! four: `grep -c "| OPEN |"` matches only that exact literal, so
//! `| OPEN (blocked on X) |` — a shape the register already uses
//! (`E-004`'s status cell is `ACCEPTED (v1.0 limitation)`) — counts as
//! zero. Fixing Gate 7 alone would still leave three more parsers free to
//! drift from it and from each other, which is E-015's family arriving by
//! way of E-014's fix (the RFC's own §7 framing).
//!
//! **Shape 3 — generate the table from structured data so nothing needs
//! parsing at all — is not built here.** It is the only shape that removes
//! the problem rather than centralising it, and it was named as the
//! architecturally better answer in both the RFC and its handoff. It is
//! also, by a wide margin, the largest change on offer: every erratum
//! entry in `ERRATA.md` is free-form prose with an embedded table row, and
//! moving to structured data means either a second source of truth to
//! keep in sync with that prose (reintroducing the drift this line exists
//! to remove) or restructuring every existing entry's authoring format —
//! genuine design work with its own review cycle, not a fix this line's
//! scope covers. Shape 1 (this module) is not a rejection of shape 3 on
//! principle; it is the answer that fits inside `Touches`.
//!
//! This module is the shared answer: one `parse_summary_rows`, reused by
//! every consumer that used to carry its own copy.

/// A tracking-column value, classified structurally (RFC-0.27-001 S1).
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Tracking {
    RfcId(String),
    Milestone(String),
    Unscheduled,
    Invalid,
}

/// Classify one tracking-column value. Pure, no I/O.
pub fn classify_tracking(raw: &str) -> Tracking {
    let raw = raw.trim();
    if raw == "unscheduled" {
        return Tracking::Unscheduled;
    }
    if let Some(rest) = raw.strip_prefix("RFC-") {
        if !rest.is_empty()
            && rest
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        {
            return Tracking::RfcId(raw.to_string());
        }
        return Tracking::Invalid;
    }
    // Bare milestone: exactly two dot-separated numeric components.
    let parts: Vec<&str> = raw.split('.').collect();
    if parts.len() == 2
        && !parts[0].is_empty()
        && !parts[1].is_empty()
        && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
    {
        return Tracking::Milestone(raw.to_string());
    }
    Tracking::Invalid
}

/// One row of the `## Summary` table: `(erratum id, raw tracking cell, raw
/// status cell)`. The status cell is kept raw — `"OPEN"`,
/// `"OPEN (blocked on X)"`, `"ACCEPTED (v1.0 limitation)"` are all valid
/// values a consumer classifies with `is_open`/`is_accepted`/`is_closed`
/// rather than an exact-string comparison.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SummaryRow {
    pub id: String,
    pub tracking: String,
    pub status: String,
}

/// Parse the `## Summary` table's rows out of a full `ERRATA.md` source.
pub fn parse_summary_rows(src: &str) -> Vec<SummaryRow> {
    let mut in_summary = false;
    let mut rows = Vec::new();
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
        rows.push(SummaryRow {
            id,
            tracking: cells[1].to_string(),
            status: cells[2].to_string(),
        });
    }
    rows
}

/// Extract a leading `E-NNN` token from a summary-table first cell such as
/// `"E-004 hardware boot"`.
pub fn extract_erratum_id(cell: &str) -> Option<String> {
    let cell = cell.trim();
    let rest = cell.strip_prefix("E-")?;
    let digit_len = rest.chars().take_while(char::is_ascii_digit).count();
    if digit_len == 0 {
        return None;
    }
    Some(format!("E-{}", &rest[..digit_len]))
}

/// A status cell counts as `OPEN` if it starts with the word `OPEN`,
/// annotation or not (RFC-0.29-002 R1/D1) — the register already writes
/// `ACCEPTED (v1.0 limitation)` in this shape, and an `OPEN` entry is
/// entitled to the same annotation without silently uncounting itself.
pub fn is_open(status: &str) -> bool {
    status.trim_start().starts_with("OPEN")
}

/// A status cell counts as `ACCEPTED` on the same structural basis.
pub fn is_accepted(status: &str) -> bool {
    status.trim_start().starts_with("ACCEPTED")
}

/// A status cell counts as `CLOSED` on the same structural basis.
pub fn is_closed(status: &str) -> bool {
    status.trim_start().starts_with("CLOSED")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_rfc_ids() {
        assert_eq!(
            classify_tracking("RFC-0.26-004"),
            Tracking::RfcId("RFC-0.26-004".to_string())
        );
        assert_eq!(
            classify_tracking("RFC-v0.16-001"),
            Tracking::RfcId("RFC-v0.16-001".to_string())
        );
    }

    #[test]
    fn classifies_bare_milestones() {
        assert_eq!(
            classify_tracking("0.27"),
            Tracking::Milestone("0.27".to_string())
        );
    }

    #[test]
    fn classifies_unscheduled() {
        assert_eq!(classify_tracking("unscheduled"), Tracking::Unscheduled);
    }

    #[test]
    fn rejects_prose() {
        assert_eq!(
            classify_tracking("0.25 candidate (recorded, not fixed)"),
            Tracking::Invalid
        );
    }

    #[test]
    fn extract_erratum_id_stops_at_first_non_digit() {
        assert_eq!(
            extract_erratum_id("E-004 hardware boot"),
            Some("E-004".to_string())
        );
        assert_eq!(extract_erratum_id("not an id"), None);
    }

    #[test]
    fn parses_summary_rows() {
        let src = "\n## Summary\n\n| Errata | Tracking RFC | Status |\n|--------|--------------|--------|\n| E-004 hardware boot | RFC-v0.16-005 | ACCEPTED (v1.0 limitation) |\n";
        let rows = parse_summary_rows(src);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "E-004");
        assert_eq!(rows[0].status, "ACCEPTED (v1.0 limitation)");
    }

    /// The demonstration Gate 7 required (RFC-0.29-002 R1): an annotated
    /// `OPEN` status, exactly the shape the register already uses for
    /// `ACCEPTED`, must still count as open.
    #[test]
    fn is_open_recognises_an_annotated_open() {
        assert!(is_open("OPEN (blocked on X)"));
        assert!(is_open("OPEN"));
        assert!(!is_open("ACCEPTED (v1.0 limitation)"));
        assert!(!is_open("CLOSED"));
    }

    #[test]
    fn is_accepted_recognises_an_annotated_accepted() {
        assert!(is_accepted("ACCEPTED (v1.0 limitation)"));
        assert!(is_accepted("ACCEPTED"));
        assert!(!is_accepted("OPEN"));
    }

    #[test]
    fn is_closed_recognises_plain_closed() {
        assert!(is_closed("CLOSED"));
        assert!(is_closed("CLOSED by RFC-0.29-001"));
        assert!(!is_closed("ACCEPTED"));
    }
}
