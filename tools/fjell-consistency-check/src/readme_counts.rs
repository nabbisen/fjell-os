//! The five counts `rfcs/README.md` asserts about the lifecycle folders, and the **one**
//! rule that compares them with the folders (0.44 small item D; E-030/E-037's hazard).
//!
//! This used to exist twice: in the `doc-counts` subcheck (a binary module, so nothing else
//! could call it) and, restated, in `crates/fjell-tools`' `rfc promote` pre-flight. Two
//! implementations of one rule drift; the restatement was held to the current tree only by a
//! test. It lives here, in the library half of this crate, and both call it.
//!
//! **The five counts** (RFC-0.27-001):
//!   1. `## Implemented (done/) — N files` -- every `.md` in `done/`;
//!   2. `N RFCs plus \`v0.7.x-index.md\`` -- that total minus one;
//!   3. `## Accepted (accepted/) — N RFCs` (or `none`);
//!   4. `## Proposed (proposed/) — N RFCs` (or `none`);
//!   5. `## Archive (archive/) — N RFCs`.
//!
//! A folder's own `README.md` (and `done/`'s `v0.7.x-index.md`) is an index of the RFCs in
//! it, not one of them, and is excluded from counts 3-5 / 2.
//!
//! **Fails closed.** A claim that cannot be found or parsed is a *disagreement*, not a skip:
//! [`disagreements`] returns a line for it. A caller that gets an empty list has been told the
//! README states all five counts and each equals the folder -- nothing weaker.

use std::fs;
use std::path::Path;

pub const DONE_DIR: &str = "rfcs/done";
pub const ACCEPTED_DIR: &str = "rfcs/accepted";
pub const PROPOSED_DIR: &str = "rfcs/proposed";
pub const ARCHIVE_DIR: &str = "rfcs/archive";

/// A folder's non-RFC index file, if it has one, excluded from its "N RFCs" count the same
/// way `v0.7.x-index.md` is excluded from `done/`'s.
pub const NON_RFC_FILES: &[&str] = &["README.md", "v0.7.x-index.md"];

/// What the folders actually hold.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct FolderCounts {
    /// Every `.md` file in `done/` (the index page included -- count 2 subtracts it).
    pub done_total: usize,
    pub accepted_rfcs: usize,
    pub proposed_rfcs: usize,
    pub archive_rfcs: usize,
}

fn count_md_files(dir: &Path) -> std::io::Result<usize> {
    Ok(fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
        .count())
}

/// How many of [`NON_RFC_FILES`] actually exist in `dir` (0 or 1 in practice today, but
/// counted rather than assumed).
fn non_rfc_present(dir: &Path) -> usize {
    NON_RFC_FILES
        .iter()
        .filter(|f| dir.join(f).is_file())
        .count()
}

/// Count the four folders under `root` (the repository root). An unreadable folder is an
/// `Err` naming it: a count that could not be taken is not zero.
pub fn count_folders(root: &Path) -> Result<FolderCounts, String> {
    let take = |rel: &str| -> Result<(usize, usize), String> {
        let dir = root.join(rel);
        let n = count_md_files(&dir).map_err(|e| format!("cannot read {rel}: {e}"))?;
        Ok((n, non_rfc_present(&dir)))
    };
    let (d, _) = take(DONE_DIR)?;
    let (a, a_non) = take(ACCEPTED_DIR)?;
    let (p, p_non) = take(PROPOSED_DIR)?;
    let (ar, ar_non) = take(ARCHIVE_DIR)?;
    Ok(FolderCounts {
        done_total: d,
        accepted_rfcs: a.saturating_sub(a_non),
        proposed_rfcs: p.saturating_sub(p_non),
        archive_rfcs: ar.saturating_sub(ar_non),
    })
}

/// Every way `readme_src`'s five counts disagree with `actual`, one line each; **empty if and
/// only if** all five are stated, parse, and match. The wording is `doc-counts`' own.
pub fn disagreements(readme_src: &str, actual: &FolderCounts) -> Vec<String> {
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

    for (label, folder, n) in [
        ("Accepted", "accepted/", actual.accepted_rfcs),
        ("Proposed", "proposed/", actual.proposed_rfcs),
        ("Archive", "archive/", actual.archive_rfcs),
    ] {
        match parse_folder_header(readme_src, label, folder) {
            Some(claimed) if claimed == n => {}
            Some(claimed) => problems.push(format!(
                "`## {label} ({folder})` claims {claimed}; actual is {n}"
            )),
            None => problems.push(format!(
                "could not find `## {label} ({folder}) — N RFCs` header"
            )),
        }
    }
    problems
}

/// Parse `## Implemented (done/) — N files`.
pub fn parse_done_header(src: &str) -> Option<usize> {
    let line = src
        .lines()
        .find(|l| l.starts_with("## Implemented (done/)"))?;
    let dash = line.rfind('—')?;
    let rest = line[dash + '—'.len_utf8()..].trim();
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Parse `N RFCs plus \`v0.7.x-index.md\``.
pub fn parse_done_rfc_count(src: &str) -> Option<usize> {
    let line = src.lines().find(|l| l.contains("RFCs plus"))?;
    let digits: String = line.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Parse `## <Label> (<folder>) — N RFCs` or `## <Label> (<folder>) — none`.
/// Returns `Some(0)` for `none`.
pub fn parse_folder_header(src: &str, label: &str, folder: &str) -> Option<usize> {
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

    fn matching() -> FolderCounts {
        FolderCounts {
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
    fn matching_counts_have_no_disagreement() {
        assert!(disagreements(fixture_readme(), &matching()).is_empty());
    }

    /// The recorded incident: the index's file count drifted from the tree (162 -> 166,
    /// corrected by hand rather than caught by any check).
    #[test]
    fn drifted_done_count_is_a_disagreement() {
        let a = FolderCounts {
            done_total: 170,
            ..matching()
        };
        let d = disagreements(fixture_readme(), &a);
        // both the header AND the derived "N RFCs plus" line disagree
        assert_eq!(d.len(), 2, "{d:?}");
        assert!(d[0].contains("claims 166 files; actual is 170"), "{d:?}");
    }

    #[test]
    fn drifted_accepted_count_is_a_disagreement() {
        let a = FolderCounts {
            accepted_rfcs: 3,
            ..matching()
        };
        assert_eq!(
            disagreements(fixture_readme(), &a),
            ["`## Accepted (accepted/)` claims 2; actual is 3"]
        );
    }

    #[test]
    fn proposed_gaining_an_rfc_without_updating_none_is_a_disagreement() {
        let a = FolderCounts {
            proposed_rfcs: 1,
            ..matching()
        };
        assert_eq!(disagreements(fixture_readme(), &a).len(), 1);
    }

    /// FAIL CLOSED: a claim that cannot be found is a disagreement, one per missing claim.
    /// This is the property `rfc promote` relies on when it refuses on a README shape it
    /// cannot parse -- it is now this function's property, in one place.
    #[test]
    fn an_unparseable_or_missing_claim_is_a_disagreement_not_a_skip() {
        let d = disagreements("# nothing here\n", &matching());
        assert_eq!(d.len(), 5, "{d:?}");
        assert!(d.iter().all(|l| l.starts_with("could not find")), "{d:?}");
        // each claim, removed alone, is exactly one disagreement that names it
        for (needle, expect) in [
            (
                "## Implemented (done/) — 166 files\n",
                "`## Implemented (done/) — N files`",
            ),
            (
                "165 RFCs plus `v0.7.x-index.md`, an overview page.\n",
                "RFCs plus",
            ),
            (
                "## Accepted (accepted/) — 2 RFCs\n",
                "## Accepted (accepted/)",
            ),
            (
                "## Proposed (proposed/) — none\n",
                "## Proposed (proposed/)",
            ),
            ("## Archive (archive/) — 1 RFC\n", "## Archive (archive/)"),
        ] {
            let src = fixture_readme().replace(needle, "");
            let d = disagreements(&src, &matching());
            assert_eq!(d.len(), 1, "removing {needle:?}: {d:?}");
            assert!(d[0].contains(expect), "{d:?}");
        }
        // a heading that is present but states no number is unparseable too
        let src = fixture_readme().replace("— 2 RFCs", "— several");
        assert!(disagreements(&src, &matching())[0].starts_with("could not find"));
    }

    /// Non-RFC index files are excluded from counts 3-5 on a real directory.
    #[test]
    fn count_folders_excludes_index_pages_and_names_an_unreadable_folder() {
        let root = std::env::temp_dir().join(format!("fjell-readmecounts-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for d in [DONE_DIR, ACCEPTED_DIR, PROPOSED_DIR, ARCHIVE_DIR] {
            fs::create_dir_all(root.join(d)).unwrap();
        }
        fs::write(root.join(DONE_DIR).join("a.md"), "x").unwrap();
        fs::write(root.join(DONE_DIR).join("v0.7.x-index.md"), "x").unwrap();
        fs::write(root.join(ACCEPTED_DIR).join("README.md"), "x").unwrap();
        fs::write(root.join(ACCEPTED_DIR).join("RFC-1.md"), "x").unwrap();
        fs::write(root.join(ARCHIVE_DIR).join("README.md"), "x").unwrap();
        assert_eq!(
            count_folders(&root).unwrap(),
            FolderCounts {
                done_total: 2, // the index page counts toward the total...
                accepted_rfcs: 1,
                proposed_rfcs: 0,
                archive_rfcs: 0,
            }
        );
        fs::remove_dir_all(root.join(PROPOSED_DIR)).unwrap();
        let e = count_folders(&root).unwrap_err();
        assert!(e.contains("cannot read rfcs/proposed"), "{e}");
        let _ = fs::remove_dir_all(&root);
    }
}
