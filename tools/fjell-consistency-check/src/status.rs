//! Shared status-keyword parsing for `rfc-status-folder` and
//! `handoff-status` (Slice 4, RFC-v0.22-001).
//!
//! RFC 000 (`rfcs/done/000-rfc-lifecycle-policy.md`) names the fixed set of
//! lifecycle states. Longest names are listed first so `contains` matching
//! finds `Implemented-with-Errata` rather than stopping at the `Implemented`
//! prefix.

pub const STATUS_KEYWORDS: &[&str] = &[
    "Implemented-with-Errata",
    "Implemented",
    "Proposed",
    "Accepted",
    "Superseded",
    "Withdrawn",
    "Closed",
];

/// Find the first line containing a bold `Status:` or `Status.` field label
/// (with or without a leading `## ` heading marker) and return the status
/// keyword found on that line. Returns `None` if no such field line exists
/// at all — a documented, deliberate exception for a handful of files (the
/// RFC lifecycle policy document itself, and the v0.7.x patch-set index)
/// that carry no Status field by design.
pub fn extract_status_keyword(src: &str) -> Option<&'static str> {
    let line = src
        .lines()
        .find(|l| l.contains("**Status:**") || l.contains("**Status.**"))?;
    STATUS_KEYWORDS
        .iter()
        .find(|kw| line.contains(*kw))
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_status_after_colon_label() {
        let src = "# Title\n\n**Status:** Implemented (v0.21.3)\n";
        assert_eq!(extract_status_keyword(src), Some("Implemented"));
    }

    #[test]
    fn finds_status_after_period_label_in_heading() {
        let src = "# Title\n\n## **Status.** Implemented (v0.9.0)\n";
        assert_eq!(extract_status_keyword(src), Some("Implemented"));
    }

    #[test]
    fn prefers_longest_match_implemented_with_errata() {
        let src = "**Status:** Implemented-with-Errata (see ERRATA E-002)\n";
        assert_eq!(extract_status_keyword(src), Some("Implemented-with-Errata"));
    }

    #[test]
    fn returns_none_when_no_status_field_present() {
        let src = "# RFC Lifecycle Policy\n\n## Status model\n\n| Status | Meaning |\n";
        assert_eq!(extract_status_keyword(src), None);
    }

    #[test]
    fn finds_proposed_even_when_accepted_mentioned_lowercase() {
        let src =
            "**Status:** Proposed — **accepted for implementation by the owner**, 2026-07-31\n";
        assert_eq!(extract_status_keyword(src), Some("Proposed"));
    }
}
