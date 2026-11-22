//! Slice 4 (RFC-v0.22-001): the `rfc-status-folder` subcheck.
//!
//! **The folder is the source of truth for an RFC's lifecycle state** — RFC
//! 000, § Folder layout. `proposed/` holds RFCs under review (`Proposed`);
//! `accepted/` holds RFCs signed off but not yet shipped (`Accepted`);
//! `done/` holds RFCs whose disposition is final (`Implemented`,
//! `Implemented-with-Errata`, `Superseded`, `Withdrawn`, `Closed`) — this
//! project has never formally withdrawn or superseded an RFC, so `Withdrawn`
//! and `Superseded` currently live in `done/` alongside `Implemented` ones;
//! `archive/` exists (RFC-0.25-002 R1) but is not read by this check, since
//! nothing has ever needed to move there.
//!
//! This citation was false from RFC-v0.22-001 until RFC-0.25-002 R2: RFC 000
//! mentioned folders zero times, and this comment (and `rfcs/README.md`)
//! cited it anyway. The citation was removed rather than left standing, and
//! is restored here now that RFC 000's merged successor states the rule
//! this comment was already assuming. A `Status:` field that disagrees with
//! its own folder is the "status field that lies" anti-pattern RFC 000
//! exists to prevent.
//!
//! A handful of files carry no `Status:` field at all, by documented
//! design (the lifecycle policy document itself, and the v0.7.x patch-set
//! index page) — those are skipped, not failed.

use std::fs;
use std::process::ExitCode;

const PROPOSED_DIR: &str = "rfcs/proposed";
const ACCEPTED_DIR: &str = "rfcs/accepted";
const DONE_DIR: &str = "rfcs/done";

// RFC-0.25-002 R1/R3: the 5-folder variant. `proposed/` must not tolerate `Accepted` -- there is a
// folder for it now, and that tolerance would be a hole. **RFC-0.47-002: which states a folder allows is
// not listed here.** It is the `Folder` column of RFC 000's state table (`status::vocabulary`), so the
// words are legal because the policy defines them, not because a constant in this file says so.

pub fn check() -> ExitCode {
    let vocab = match crate::status::vocabulary() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("rfc-status-folder: FAIL — {e}");
            return ExitCode::FAILURE;
        }
    };
    let proposed = vocab.names_in("proposed");
    let accepted = vocab.names_in("accepted");
    let done = vocab.names_in("done");
    let mut files: Vec<(String, String, &[&str])> = Vec::new();
    for (dir, allowed) in [
        (PROPOSED_DIR, proposed.as_slice()),
        (ACCEPTED_DIR, accepted.as_slice()),
        (DONE_DIR, done.as_slice()),
    ] {
        let Some(entries) = crate::read_dir_named("rfc-status-folder", dir) else {
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
    run_check_in(vocab, &refs)
}

/// [`run_check_in`] against the project's own vocabulary.
#[cfg(test)]
pub fn run_check(files: &[(&str, &str, &[&str])]) -> ExitCode {
    match crate::status::vocabulary() {
        Ok(v) => run_check_in(v, files),
        Err(e) => {
            eprintln!("rfc-status-folder: FAIL — {e}");
            ExitCode::FAILURE
        }
    }
}

/// Core comparison, pure in its inputs for testing with synthetic fixtures.
/// Each tuple is `(path label, file content, statuses allowed in that folder)`.
///
/// **A Status field that names no state the policy defines is a failure.** It used to be skipped as if the
/// file had no Status field (the documented exception), so a status word the policy did not define -- or a
/// defined one misspelled -- passed unseen.
pub fn run_check_in(
    vocab: &crate::status::Vocabulary,
    files: &[(&str, &str, &[&str])],
) -> ExitCode {
    let mut problems = Vec::new();
    let mut checked = 0usize;
    for (path, content, allowed) in files {
        let Some(line) = crate::status::status_field_line(content) else {
            continue; // documented exception: no Status field at all
        };
        let Some(found) = vocab.keyword_in(line) else {
            problems.push(format!(
                "{path}: Status field `{}` names no state RFC 000 defines",
                line.trim()
            ));
            continue;
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

    /// The folders' allowed sets, from the real policy (the same table the check reads).
    fn allowed(folder: &str) -> &'static [&'static str] {
        let v = crate::status::vocabulary().expect("policy parses");
        Box::leak(v.names_in(folder).into_boxed_slice())
    }

    #[test]
    fn proposed_rfc_with_proposed_status_passes() {
        let files = [(
            "rfcs/proposed/X.md",
            "**Status:** Proposed\n",
            allowed("proposed"),
        )];
        assert_eq!(run_check(&files), ExitCode::SUCCESS);
    }

    #[test]
    fn done_rfc_with_implemented_status_passes() {
        let files = [(
            "rfcs/done/X.md",
            "**Status:** Implemented (v0.5.0)\n",
            allowed("done"),
        )];
        assert_eq!(run_check(&files), ExitCode::SUCCESS);
    }

    /// RFC-0.25-002 R3 failure demonstration (1 of 2): an `Accepted` RFC left
    /// behind in `proposed/`. Legal before the 5-folder variant, a hole after.
    #[test]
    fn accepted_rfc_left_in_proposed_fails() {
        let files = [(
            "rfcs/proposed/X.md",
            "**Status:** Accepted — by the owner, 2026-08-03\n",
            allowed("proposed"),
        )];
        assert_eq!(run_check(&files), ExitCode::FAILURE);
    }

    /// RFC-0.25-002 R3 failure demonstration (2 of 2): a `Proposed` RFC placed
    /// in `accepted/` — the opposite direction, which a one-way check misses.
    #[test]
    fn proposed_rfc_placed_in_accepted_fails() {
        let files = [(
            "rfcs/accepted/X.md",
            "**Status:** Proposed — awaiting owner acceptance\n",
            allowed("accepted"),
        )];
        assert_eq!(run_check(&files), ExitCode::FAILURE);
    }

    #[test]
    fn accepted_rfc_in_accepted_passes() {
        let files = [(
            "rfcs/accepted/X.md",
            "**Status:** Accepted — by the owner, 2026-08-03\n",
            allowed("accepted"),
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
            allowed("done"),
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
            allowed("proposed"),
        )];
        assert_eq!(run_check(&files), ExitCode::FAILURE);
    }

    /// RFC-0.47-002: a status word the policy does not define used to be skipped like a missing field.
    #[test]
    fn a_status_word_the_policy_does_not_define_fails() {
        for word in ["Closed (all done)", "Draft", "Impemented"] {
            let content = format!("**Status:** {word}\n");
            let files = [("rfcs/done/X.md", content.as_str(), allowed("done"))];
            assert_eq!(run_check(&files), ExitCode::FAILURE, "{word}");
        }
    }

    #[test]
    fn files_with_no_status_field_are_skipped_not_failed() {
        let files = [(
            "rfcs/done/000-rfc-lifecycle-policy.md",
            "# no status field here\n",
            allowed("done"),
        )];
        assert_eq!(run_check(&files), ExitCode::SUCCESS);
    }
}
