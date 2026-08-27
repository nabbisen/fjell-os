//! RFC-0.27-001: the `doc-counts` subcheck (S5).
//!
//! `rfcs/README.md` asserts, in prose, how many files live in each RFC
//! lifecycle folder. Those counts drifted from 162 to 166 and were
//! corrected by hand on 2026-08-27 — the same shape of defect as the
//! errata tracking column and the README version badge, just in a third
//! document.
//!
//! ## Scope: five named counts, not "every number in every document"
//!
//! A general "verify every asserted number" check is not buildable — most
//! numbers in this project's documentation are measurements, illustrative
//! examples, or historical figures with no single mechanically-derivable
//! source of truth (see `version_currency`'s design note for the sibling
//! problem in S3). This checks exactly the five counts named in the RFC
//! (`rfcs/README.md`'s folder headers), each mechanically derivable as "the
//! number of files in a directory":
//!
//!   1. `## Implemented (done/) — N files` — total `.md` files in `done/`.
//!   2. `"N RFCs plus \`v0.7.x-index.md\`"` — the same total minus the one
//!      documented non-RFC file in that folder.
//!   3. `## Accepted (accepted/) — N RFCs`
//!   4. `## Proposed (proposed/) — none` (or `N RFCs`)
//!   5. `## Archive (archive/) — N RFCs`
//!
//! `README.md` (the file, present in `accepted/`, `proposed/`, and
//! `archive/` as a folder-local index page — mirroring `done/`'s
//! `v0.7.x-index.md`) is excluded from counts 3–5 the same way
//! `v0.7.x-index.md` is excluded from count 2: it is an index of the RFCs
//! in that folder, not one of them.
//!
//! **Excluded deliberately**, per the handoff's instruction to say so: the
//! instrument audit's own totals table (`docs/verification/instrument-audit.md`
//! — already corrected under RFC-0.24-003, and its own document, not
//! `rfcs/README.md`), and every other numeric claim anywhere else in the
//! tree (service counts, syscall counts already covered by
//! `syscall-surface`, line counts in commentary, etc.) — none of those are
//! "a document asserting a count about the tree" in the same
//! directly-derivable sense; each would need its own bespoke parser for a
//! single data point, which is the general-linter shape this RFC's Scope
//! explicitly rules out.

use crate::read_file;
use std::fs;
use std::process::ExitCode;

const README_PATH: &str = "rfcs/README.md";
const DONE_DIR: &str = "rfcs/done";
const ACCEPTED_DIR: &str = "rfcs/accepted";
const PROPOSED_DIR: &str = "rfcs/proposed";
const ARCHIVE_DIR: &str = "rfcs/archive";

/// A folder's non-RFC index file, if it has one, excluded from its "N RFCs"
/// count the same way `v0.7.x-index.md` is excluded from `done/`'s.
const NON_RFC_FILES: &[&str] = &["README.md", "v0.7.x-index.md"];

pub fn check() -> ExitCode {
    let Some(readme_src) = read_file(README_PATH) else {
        return ExitCode::FAILURE;
    };
    let counts = match (
        count_md_files(DONE_DIR),
        count_md_files(ACCEPTED_DIR),
        count_md_files(PROPOSED_DIR),
        count_md_files(ARCHIVE_DIR),
    ) {
        (Ok(d), Ok(a), Ok(p), Ok(ar)) => ActualCounts {
            done_total: d,
            accepted_rfcs: a.saturating_sub(non_rfc_present(ACCEPTED_DIR)),
            proposed_rfcs: p.saturating_sub(non_rfc_present(PROPOSED_DIR)),
            archive_rfcs: ar.saturating_sub(non_rfc_present(ARCHIVE_DIR)),
        },
        _ => {
            eprintln!("consistency-check: cannot read one or more rfcs/ lifecycle folders");
            return ExitCode::FAILURE;
        }
    };
    run_check(&readme_src, &counts)
}

fn count_md_files(dir: &str) -> std::io::Result<usize> {
    Ok(fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
        .count())
}

/// How many of `NON_RFC_FILES` actually exist in `dir` (0 or 1 in practice
/// today, but counted rather than assumed).
fn non_rfc_present(dir: &str) -> usize {
    NON_RFC_FILES
        .iter()
        .filter(|f| std::path::Path::new(dir).join(f).is_file())
        .count()
}

#[derive(Debug, PartialEq, Eq)]
pub struct ActualCounts {
    pub done_total: usize,
    pub accepted_rfcs: usize,
    pub proposed_rfcs: usize,
    pub archive_rfcs: usize,
}

/// Core comparison, pure in its inputs for testing with synthetic fixtures.
pub fn run_check(readme_src: &str, actual: &ActualCounts) -> ExitCode {
    let mut problems = Vec::new();

    match parse_done_header(readme_src) {
        Some(claimed) if claimed == actual.done_total => {}
        Some(claimed) => problems.push(format!(
            "`## Implemented (done/)` claims {claimed} files; actual is {}",
            actual.done_total
        )),
        None => {
            problems.push("could not find `## Implemented (done/) — N files` header".to_string())
        }
    }

    match parse_done_rfc_count(readme_src) {
        Some(claimed) if claimed == actual.done_total.saturating_sub(1) => {}
        Some(claimed) => problems.push(format!(
            "\"{claimed} RFCs plus `v0.7.x-index.md`\" claims {claimed}; actual (total minus the index page) is {}",
            actual.done_total.saturating_sub(1)
        )),
        None => problems.push("could not find the \"N RFCs plus `v0.7.x-index.md`\" line".to_string()),
    }

    check_folder_count(
        readme_src,
        "Accepted",
        "accepted/",
        actual.accepted_rfcs,
        &mut problems,
    );
    check_folder_count(
        readme_src,
        "Proposed",
        "proposed/",
        actual.proposed_rfcs,
        &mut problems,
    );
    check_folder_count(
        readme_src,
        "Archive",
        "archive/",
        actual.archive_rfcs,
        &mut problems,
    );

    if problems.is_empty() {
        println!("doc-counts: PASS (5 counts in {README_PATH} match the tree)");
        ExitCode::SUCCESS
    } else {
        eprintln!("doc-counts: FAIL");
        for p in &problems {
            eprintln!("  {p}");
        }
        ExitCode::FAILURE
    }
}

fn check_folder_count(
    src: &str,
    label: &str,
    folder: &str,
    actual: usize,
    problems: &mut Vec<String>,
) {
    match parse_folder_header(src, label, folder) {
        Some(claimed) if claimed == actual => {}
        Some(claimed) => problems.push(format!(
            "`## {label} ({folder})` claims {claimed}; actual is {actual}"
        )),
        None => problems.push(format!(
            "could not find `## {label} ({folder}) — N RFCs` header"
        )),
    }
}

/// Parse `## Implemented (done/) — N files`.
fn parse_done_header(src: &str) -> Option<usize> {
    let line = src
        .lines()
        .find(|l| l.starts_with("## Implemented (done/)"))?;
    let dash = line.rfind('—')?;
    let rest = line[dash + '—'.len_utf8()..].trim();
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Parse `N RFCs plus \`v0.7.x-index.md\``.
fn parse_done_rfc_count(src: &str) -> Option<usize> {
    let line = src.lines().find(|l| l.contains("RFCs plus"))?;
    let digits: String = line.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Parse `## <Label> (<folder>) — N RFCs` or `## <Label> (<folder>) — none`.
/// Returns `Some(0)` for `none`.
fn parse_folder_header(src: &str, label: &str, folder: &str) -> Option<usize> {
    let prefix = format!("## {label} ({folder})");
    let line = src.lines().find(|l| l.starts_with(&prefix))?;
    let dash = line.rfind('—')?;
    let rest = line[dash + '—'.len_utf8()..].trim();
    if rest.eq_ignore_ascii_case("none") {
        return Some(0);
    }
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_readme() -> &'static str {
        "## Implemented (done/) — 166 files\n\n\
         165 RFCs plus `v0.7.x-index.md`, an overview page.\n\n\
         ## Accepted (accepted/) — 2 RFCs\n\n\
         ## Proposed (proposed/) — none\n\n\
         ## Archive (archive/) — 1 RFC\n"
    }

    fn matching_counts() -> ActualCounts {
        ActualCounts {
            done_total: 166,
            accepted_rfcs: 2,
            proposed_rfcs: 0,
            archive_rfcs: 1,
        }
    }

    #[test]
    fn parses_done_header() {
        assert_eq!(parse_done_header(fixture_readme()), Some(166));
    }

    #[test]
    fn parses_done_rfc_count() {
        assert_eq!(parse_done_rfc_count(fixture_readme()), Some(165));
    }

    #[test]
    fn parses_none_as_zero() {
        assert_eq!(
            parse_folder_header(fixture_readme(), "Proposed", "proposed/"),
            Some(0)
        );
    }

    #[test]
    fn parses_numeric_folder_count() {
        assert_eq!(
            parse_folder_header(fixture_readme(), "Accepted", "accepted/"),
            Some(2)
        );
        assert_eq!(
            parse_folder_header(fixture_readme(), "Archive", "archive/"),
            Some(1)
        );
    }

    #[test]
    fn matching_counts_pass() {
        assert_eq!(
            run_check(fixture_readme(), &matching_counts()),
            ExitCode::SUCCESS
        );
    }

    /// Required failure demonstration: the recorded incident — the index's
    /// file count drifted from the actual tree (162 → 166, corrected by
    /// hand rather than caught by any check).
    #[test]
    fn drifted_done_count_fails() {
        let actual = ActualCounts {
            done_total: 170, // tree grew; README.md was not updated
            ..matching_counts()
        };
        assert_eq!(run_check(fixture_readme(), &actual), ExitCode::FAILURE);
    }

    #[test]
    fn drifted_accepted_count_fails() {
        let actual = ActualCounts {
            accepted_rfcs: 3,
            ..matching_counts()
        };
        assert_eq!(run_check(fixture_readme(), &actual), ExitCode::FAILURE);
    }

    #[test]
    fn proposed_folder_gaining_an_rfc_without_updating_none_fails() {
        let actual = ActualCounts {
            proposed_rfcs: 1, // an RFC was added; header still says "none"
            ..matching_counts()
        };
        assert_eq!(run_check(fixture_readme(), &actual), ExitCode::FAILURE);
    }
}
