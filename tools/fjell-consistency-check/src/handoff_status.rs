//! Slice 4 (RFC-v0.22-001): the `handoff-status` subcheck.
//!
//! Each `rfcs/handoffs/<RFC>/implementation-handoff.md` declares its Status
//! as "inherited from the governing RFC". This subcheck verifies the
//! inheritance actually holds: the handoff's own Status keyword must match
//! its governing RFC's current Status keyword. This is exactly RFC-v0.22-001
//! Motivation instance 4 — a handoff stayed `Proposed` after its governing
//! RFC moved to `Implemented`.
//!
//! RFC-0.30-002 R2 diagnosis: E-038's own repro table describes this
//! subcheck as printing "nothing" when `rfcs/accepted/` is moved aside.
//! Reproduced directly, twice, against the exact commits involved, and
//! that is not what happens: it either (a) prints a real, if unnamed,
//! `... links to governing RFC ... which could not be read` message, if
//! some live handoff's Governing RFC link currently resolves into the
//! missing folder, or (b) **passes** — incorrectly — if none currently
//! does. (b) is the real defect, and it is worse than silence: unlike
//! `rfc-status-folder`/`errata-tracking`/`doc-counts`, which enumerate
//! `rfcs/{proposed,accepted,done}` directly and so notice a missing one
//! unconditionally, this subcheck only ever touched those folders
//! incidentally, through whichever RFC a handoff happened to cite. Right
//! after a release cut — the exact moment `rfcs/accepted/` is emptiest and
//! likeliest to vanish from a fresh clone, per E-038's own history — no
//! handoff yet cites anything in it, and this subcheck would pass on a
//! clone missing the folder entirely. Fixed below by checking all three
//! lifecycle folders directly, matching the others, rather than waiting to
//! notice one by accident.

use crate::status::extract_status_keyword;
use std::fs;
use std::process::ExitCode;

const PROPOSED_DIR: &str = "rfcs/proposed";
const ACCEPTED_DIR: &str = "rfcs/accepted";
const DONE_DIR: &str = "rfcs/done";
const HANDOFFS_DIR: &str = "rfcs/handoffs";
const NAME: &str = "handoff-status";

pub fn check() -> ExitCode {
    // A Governing RFC link can resolve into any of the three lifecycle
    // folders depending on where that RFC currently lives. Verify all
    // three are readable unconditionally, the same way the other three
    // E-038 subchecks do — not only when a handoff's link happens to touch
    // one of them.
    for dir in [PROPOSED_DIR, ACCEPTED_DIR, DONE_DIR] {
        if crate::read_dir_named(NAME, dir).is_none() {
            return ExitCode::FAILURE;
        }
    }

    let Some(entries) = crate::read_dir_named(NAME, HANDOFFS_DIR) else {
        return ExitCode::FAILURE;
    };
    let mut dirs: Vec<_> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();

    let mut pairs: Vec<(String, String, String)> = Vec::new();
    for dir in dirs {
        let handoff_path = dir.join("implementation-handoff.md");
        let Some(handoff_src) = crate::read_file(NAME, &handoff_path.to_string_lossy()) else {
            return ExitCode::FAILURE;
        };
        let Some(rel_link) = extract_governing_rfc_link(&handoff_src) else {
            eprintln!(
                "{NAME}: FAIL — {} has no '**Governing RFC:**' link",
                handoff_path.display()
            );
            return ExitCode::FAILURE;
        };
        let governing_path = dir.join(&rel_link);
        let Ok(governing_src) = fs::read_to_string(&governing_path) else {
            eprintln!(
                "{NAME}: FAIL — {} links to governing RFC {} which could not be read",
                handoff_path.display(),
                governing_path.display()
            );
            return ExitCode::FAILURE;
        };
        pairs.push((
            handoff_path.to_string_lossy().to_string(),
            handoff_src,
            governing_src,
        ));
    }
    let refs: Vec<(&str, &str, &str)> = pairs
        .iter()
        .map(|(p, h, g)| (p.as_str(), h.as_str(), g.as_str()))
        .collect();
    run_check(&refs)
}

/// Core comparison, pure in its inputs for testing with synthetic fixtures.
/// Each tuple is `(handoff path label, handoff content, governing RFC content)`.
pub fn run_check(pairs: &[(&str, &str, &str)]) -> ExitCode {
    let mut problems = Vec::new();
    for (path, handoff_src, governing_src) in pairs {
        let Some(handoff_status) = extract_status_keyword(handoff_src) else {
            problems.push(format!("{path}: no Status field found in the handoff"));
            continue;
        };
        let Some(governing_status) = extract_status_keyword(governing_src) else {
            problems.push(format!(
                "{path}: governing RFC has no Status field to inherit"
            ));
            continue;
        };
        if handoff_status != governing_status {
            problems.push(format!(
                "{path}: handoff Status is {handoff_status:?} but governing RFC Status is {governing_status:?}"
            ));
        }
    }
    if problems.is_empty() {
        println!("handoff-status: PASS ({} handoffs checked)", pairs.len());
        ExitCode::SUCCESS
    } else {
        eprintln!("handoff-status: FAIL");
        for p in &problems {
            eprintln!("  {p}");
        }
        ExitCode::FAILURE
    }
}

/// Parse `**Governing RFC:** [label](relative/path.md)` and return the
/// relative path, which is resolved relative to the handoff file's own
/// directory by the caller.
fn extract_governing_rfc_link(src: &str) -> Option<String> {
    let line = src.lines().find(|l| l.contains("**Governing RFC:**"))?;
    let open = line.find('(')?;
    let close = line[open..].find(')')? + open;
    Some(line[open + 1..close].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_governing_rfc_relative_link() {
        let src =
            "**Governing RFC:** [RFC-v0.22-001](../../proposed/RFC-v0.22-001-gate-integrity.md)\n";
        assert_eq!(
            extract_governing_rfc_link(src),
            Some("../../proposed/RFC-v0.22-001-gate-integrity.md".to_string())
        );
    }

    #[test]
    fn matching_status_passes() {
        let handoff = "**Status:** inherited from the governing RFC — **Implemented (v0.21.3)**\n";
        let governing = "**Status:** Implemented (v0.21.3)\n";
        let pairs = [("h.md", handoff, governing)];
        assert_eq!(run_check(&pairs), ExitCode::SUCCESS);
    }

    /// Required failure demonstration: the recorded live instance — a
    /// handoff whose inherited Status stayed `Proposed` after its
    /// governing RFC moved to `Implemented`.
    #[test]
    fn stale_proposed_handoff_after_rfc_implemented_fails() {
        let handoff = "**Status:** inherited from the governing RFC (Proposed — under review)\n";
        let governing = "**Status:** Implemented (v0.21.3)\n";
        let pairs = [("h.md", handoff, governing)];
        assert_eq!(
            run_check(&pairs),
            ExitCode::FAILURE,
            "a handoff whose Status disagrees with its governing RFC must fail the check"
        );
    }

    #[test]
    fn both_proposed_passes() {
        let handoff = "**Status:** inherited from the governing RFC (Proposed — accepted for implementation)\n";
        let governing = "**Status:** Proposed — accepted for implementation\n";
        let pairs = [("h.md", handoff, governing)];
        assert_eq!(run_check(&pairs), ExitCode::SUCCESS);
    }
}
