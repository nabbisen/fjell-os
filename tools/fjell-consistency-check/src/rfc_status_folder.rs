//! Slice 4 (RFC-v0.22-001): the `rfc-status-folder` subcheck.
//!
//! `rfcs/README.md` documents the folder as the source of truth for an
//! RFC's lifecycle state (RFC 000): `proposed/` holds RFCs not yet finally
//! dispositioned (`Proposed` or `Accepted`); `done/` holds RFCs whose
//! disposition is final (`Implemented`, `Implemented-with-Errata`,
//! `Superseded`, `Withdrawn`, `Closed`). A `Status:` field that disagrees
//! with its own folder is the "status field that lies" anti-pattern RFC 000
//! exists to prevent.
//!
//! A handful of files carry no `Status:` field at all, by documented
//! design (the lifecycle policy document itself, and the v0.7.x patch-set
//! index page) — those are skipped, not failed.

use crate::status::extract_status_keyword;
use std::fs;
use std::process::ExitCode;

const PROPOSED_DIR: &str = "rfcs/proposed";
const DONE_DIR: &str = "rfcs/done";

const PROPOSED_STATUSES: &[&str] = &["Proposed", "Accepted"];
const DONE_STATUSES: &[&str] = &[
    "Implemented-with-Errata",
    "Implemented",
    "Superseded",
    "Withdrawn",
    "Closed",
];

pub fn check() -> ExitCode {
    let mut files: Vec<(String, String, &[&str])> = Vec::new();
    for (dir, allowed) in [(PROPOSED_DIR, PROPOSED_STATUSES), (DONE_DIR, DONE_STATUSES)] {
        let Ok(entries) = fs::read_dir(dir) else {
            eprintln!("consistency-check: cannot read {dir}");
            return ExitCode::FAILURE;
        };
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "md"))
            .collect();
        paths.sort();
        for path in paths {
            let content = fs::read_to_string(&path).unwrap_or_default();
            files.push((path.to_string_lossy().to_string(), content, allowed));
        }
    }
    let refs: Vec<(&str, &str, &[&str])> = files
        .iter()
        .map(|(p, c, a)| (p.as_str(), c.as_str(), *a))
        .collect();
    run_check(&refs)
}

/// Core comparison, pure in its inputs for testing with synthetic fixtures.
/// Each tuple is `(path label, file content, statuses allowed in that folder)`.
pub fn run_check(files: &[(&str, &str, &[&str])]) -> ExitCode {
    let mut problems = Vec::new();
    let mut checked = 0usize;
    for (path, content, allowed) in files {
        let Some(found) = extract_status_keyword(content) else {
            continue; // documented exception: no Status field at all
        };
        checked += 1;
        if !allowed.contains(&found) {
            problems.push(format!(
                "{path}: Status is {found:?}, but its folder only allows {allowed:?}"
            ));
        }
    }
    if problems.is_empty() {
        println!("rfc-status-folder: PASS ({checked} RFCs checked)");
        ExitCode::SUCCESS
    } else {
        eprintln!("rfc-status-folder: FAIL");
        for p in &problems {
            eprintln!("  {p}");
        }
        ExitCode::FAILURE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposed_rfc_with_proposed_status_passes() {
        let files = [(
            "rfcs/proposed/X.md",
            "**Status:** Proposed\n",
            PROPOSED_STATUSES,
        )];
        assert_eq!(run_check(&files), ExitCode::SUCCESS);
    }

    #[test]
    fn done_rfc_with_implemented_status_passes() {
        let files = [(
            "rfcs/done/X.md",
            "**Status:** Implemented (v0.5.0)\n",
            DONE_STATUSES,
        )];
        assert_eq!(run_check(&files), ExitCode::SUCCESS);
    }

    /// Required failure demonstration: the recorded live instance —
    /// an RFC filed under `rfcs/done/` whose Status field still reads
    /// `Accepted`, never updated to `Implemented`.
    #[test]
    fn done_rfc_with_accepted_status_fails() {
        let files = [(
            "rfcs/done/RFC-v0.17-001-trust-anchor-provisioning.md",
            "**Status:** Accepted (design options — requires architect decision)\n",
            DONE_STATUSES,
        )];
        assert_eq!(
            run_check(&files),
            ExitCode::FAILURE,
            "an RFC in done/ with an unfinished Status must fail the check"
        );
    }

    #[test]
    fn proposed_rfc_with_implemented_status_fails() {
        let files = [(
            "rfcs/proposed/X.md",
            "**Status:** Implemented (v0.5.0)\n",
            PROPOSED_STATUSES,
        )];
        assert_eq!(run_check(&files), ExitCode::FAILURE);
    }

    #[test]
    fn files_with_no_status_field_are_skipped_not_failed() {
        let files = [(
            "rfcs/done/000-rfc-lifecycle-policy.md",
            "# no status field here\n",
            DONE_STATUSES,
        )];
        assert_eq!(run_check(&files), ExitCode::SUCCESS);
    }
}
