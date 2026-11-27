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
//! number of files in a directory".
//!
//! **The rule itself is not in this file** (0.44 small item D). The five counts, how each is
//! parsed and compared, and how the folders are counted are
//! [`fjell_consistency_check::readme_counts`], in the library half of this crate, so that
//! `cargo xtask rfc promote`'s pre-flight calls the very same function rather than restating
//! it. This module reads the README, asks the library, and reports.
//!
//! **Excluded deliberately**, per the handoff's instruction to say so: the
//! instrument audit's own totals table (`docs/src/assurance/instrument-audit.md`
//! — already corrected under RFC-0.24-003, and its own document, not
//! `rfcs/README.md`), and every other numeric claim anywhere else in the
//! tree (service counts, line counts in commentary, etc.) — none of those are
//! "a document asserting a count about the tree" in the same
//! directly-derivable sense; each would need its own bespoke parser for a
//! single data point, which is the general-linter shape this RFC's Scope
//! explicitly rules out.
//!
//! **Who owns what the exclusion names (corrected at 0.50, RFC-0.50-002 D6).** This note used to
//! list *"syscall counts already covered by `syscall-surface`"* among the things it declined, and
//! `syscall-surface` reads three source files and a TOML and **no document** -- so the question was
//! declined on the grounds that a sibling owned it, the sibling did not, and that sentence is why
//! nobody looked again while the Syscall Reference said "26 of 35" for twenty milestones (the check
//! 0.22 named for exactly this, *"to stop documented-surface drift recurring"*, was built and given
//! the wrong inputs). Now: the **syscall tables in the documents** are `syscall-doc`'s (the
//! Syscall Reference and `kernel.md`'s group table, against `SyscallNumber`); the **dispatch state**
//! is `syscall-surface`'s; **service and driver counts, and the service table, are covered by
//! nothing** -- named here so that the next reader is not told they belong to somebody else.

use crate::read_file;
use fjell_consistency_check::readme_counts::{FolderCounts, count_folders, disagreements};
use std::path::Path;
use std::process::ExitCode;

const README_PATH: &str = "rfcs/README.md";
const NAME: &str = "doc-counts";

pub fn check() -> ExitCode {
    let Some(readme_src) = read_file(NAME, README_PATH) else {
        return ExitCode::FAILURE;
    };
    // RFC-0.30-002 R1: an unreadable folder is named, not folded into one message.
    let counts = match count_folders(Path::new(".")) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{NAME}: FAIL — {e}");
            return ExitCode::FAILURE;
        }
    };
    run_check(&readme_src, &counts)
}

/// Core comparison, pure in its inputs for testing with synthetic fixtures.
pub fn run_check(readme_src: &str, actual: &FolderCounts) -> ExitCode {
    let problems = disagreements(readme_src, actual);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC-0.50-002 D6: this module's own scope note must not claim that somebody else covers a question nobody covers. It named
    /// `syscall-surface` as the owner of syscall counts in documents while that check read no document. The note now names the check
    /// that does (`syscall-doc`) and says what is uncovered; this test pins both, and that the false sentence is gone.
    #[test]
    fn the_scope_note_names_the_check_that_owns_the_syscall_tables_and_what_nothing_covers() {
        let me = include_str!("doc_counts.rs");
        assert!(me.contains("`syscall-doc`"), "the note must name the check that reads the documents");
        assert!(me.contains("covered by\n//! nothing"), "the note must say what is uncovered");
        let false_sentence = ["syscall counts already covered", " by\n//! `syscall-surface`"].concat();
        let hits = me.matches(false_sentence.as_str()).count();
        assert_eq!(hits, 0, "the sentence that said syscall-surface covers the documents must not come back");
    }

    fn fixture_readme() -> &'static str {
        "## Implemented (done/) — 166 files\n\n\
         165 RFCs plus `v0.7.x-index.md`, an overview page.\n\n\
         ## Accepted (accepted/) — 2 RFCs\n\n\
         ## Proposed (proposed/) — none\n\n\
         ## Archive (archive/) — 1 RFC\n"
    }

    fn matching_counts() -> FolderCounts {
        FolderCounts {
            done_total: 166,
            accepted_rfcs: 2,
            proposed_rfcs: 0,
            archive_rfcs: 1,
        }
    }

    // The parsing and the comparison are tested where they live
    // (`readme_counts`); these pin that the SUBCHECK still turns a disagreement into a
    // failing exit, which is the only thing it adds.

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
        let actual = FolderCounts {
            done_total: 170, // tree grew; README.md was not updated
            ..matching_counts()
        };
        assert_eq!(run_check(fixture_readme(), &actual), ExitCode::FAILURE);
    }

    #[test]
    fn drifted_accepted_count_fails() {
        let actual = FolderCounts {
            accepted_rfcs: 3,
            ..matching_counts()
        };
        assert_eq!(run_check(fixture_readme(), &actual), ExitCode::FAILURE);
    }

    #[test]
    fn proposed_folder_gaining_an_rfc_without_updating_none_fails() {
        let actual = FolderCounts {
            proposed_rfcs: 1, // an RFC was added; header still says "none"
            ..matching_counts()
        };
        assert_eq!(run_check(fixture_readme(), &actual), ExitCode::FAILURE);
    }

    #[test]
    fn an_unparseable_readme_fails_closed() {
        assert_eq!(
            run_check("# no counts at all\n", &matching_counts()),
            ExitCode::FAILURE
        );
    }
}
