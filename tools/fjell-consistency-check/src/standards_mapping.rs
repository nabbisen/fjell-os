//! RFC-0.27-003: the `standards-mapping` subcheck (Gate 12's 9th subcheck).
//!
//! Verifies the row-level contract (RFC §R3) over
//! `docs/src/compliance/standards-mapping.md`:
//!
//!   1. every status cell is from D4's closed vocabulary: `met`, `partial`,
//!      `not-met`, `not-applicable`, `roadmap`, `unassessed`;
//!   2. every `met`, `partial`, or `unassessed` row cites at least one
//!      evidence path (an `unassessed` row's cited path is candidate
//!      evidence only, per D4, but it must still exist);
//!   3. every cited path (any row, any status) exists in the tree;
//!   4. **direction B** — a row this parser cannot read as a well-formed
//!      five-column row (wrong column count, or a status cell present but
//!      empty) FAILs by name. It is never dropped from the count because
//!      the parser could not make sense of it — a row-parser that skips
//!      malformed rows reports `PASS` on a document it did not read (RFC
//!      §4 / handoff §4), the same mode `doc-links` and `errata-tracking`
//!      avoid;
//!   5. **a structural (IEC) row cannot carry a verdict.** Review of commit
//!      `fb05a1a` found ten of fifteen IEC rows marked `met` against clause
//!      text D3 forbids reading — a verdict against unread criteria is not
//!      a status. Any row whose ID starts with `IEC-` must be `unassessed`;
//!      any other status on an `IEC-` row FAILs by name. This is the
//!      mechanical form of that ruling: it does not need document prose to
//!      hold, because the ID prefix already distinguishes clause-level (CRA)
//!      rows from structural-only (IEC) ones.
//!
//! **What this does not verify, stated here and in the mapping document
//! itself:** that a cited artifact still supports the claim in its row —
//! only that the path exists. Checking semantic support mechanically is
//! not buildable; this is a weak predicate, and it is deliberate.
//!
//! Evidence-column paths are Markdown links (`[label](path)`), written
//! relative to the mapping document's own directory — the same convention
//! `doc-links` checks for every other tracked document — so they are
//! resolved against `docs/src/compliance/`, not the repository root.

use crate::read_file;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const MAPPING_PATH: &str = "docs/src/compliance/standards-mapping.md";
const MAPPING_DIR: &str = "docs/src/compliance";
const STATUS_VALUES: &[&str] = &[
    "met",
    "partial",
    "not-met",
    "not-applicable",
    "roadmap",
    "unassessed",
];
/// A row whose ID carries this prefix maps to a standard whose clause text
/// has not been read (D3) — it may only ever be `unassessed` (rule 5).
const STRUCTURAL_ID_PREFIX: &str = "IEC-";

/// One row as read from the table. `Malformed` carries whatever the parser
/// could still identify (a line number and, where available, a first-cell
/// label) for a row it could not read as five well-formed columns — see
/// module doc point 4.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum MappingRow {
    Row {
        line: usize,
        id: String,
        status: String,
        evidence: String,
    },
    Malformed {
        line: usize,
        label: String,
        reason: String,
    },
}

/// Parse every table row in the document. A "table row" is any line whose
/// trimmed form starts with `|`; header and separator rows are recognised
/// and skipped, everything else is either a well-formed 5-column row or a
/// `Malformed` one.
pub fn parse_rows(src: &str) -> Vec<MappingRow> {
    let mut rows = Vec::new();
    for (i, line) in src.lines().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = trimmed
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();

        if is_separator_row(&cells) {
            continue;
        }
        if cells.first().is_some_and(|c| c.eq_ignore_ascii_case("id")) {
            continue; // header row
        }

        let line_no = i + 1;
        if cells.len() != 5 {
            rows.push(MappingRow::Malformed {
                line: line_no,
                label: cells.first().unwrap_or(&"<empty>").to_string(),
                reason: format!("expected 5 columns (ID | Requirement | Status | Mechanism | Evidence), found {}", cells.len()),
            });
            continue;
        }
        if cells[2].is_empty() {
            rows.push(MappingRow::Malformed {
                line: line_no,
                label: cells[0].to_string(),
                reason: "status cell is empty".to_string(),
            });
            continue;
        }
        rows.push(MappingRow::Row {
            line: line_no,
            id: cells[0].to_string(),
            status: cells[2].to_string(),
            evidence: cells[4].to_string(),
        });
    }
    rows
}

fn is_separator_row(cells: &[&str]) -> bool {
    !cells.is_empty()
        && cells
            .iter()
            .all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':'))
}

/// Extract every Markdown link target from an Evidence cell. Same bracket
/// scan as `doc_links::extract_links`, duplicated rather than shared: this
/// operates on a single table cell, not a whole document, and the two
/// checks are independent instruments by design (RFC-0.27-001's own
/// precedent — each subcheck reads the tree itself).
fn extract_evidence_paths(cell: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut i = 0;
    let bytes = cell.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'[' {
            if let Some(close_bracket) = cell[i..].find(']') {
                let after = i + close_bracket + 1;
                if cell.as_bytes().get(after) == Some(&b'(') {
                    if let Some(close_paren_rel) = cell[after..].find(')') {
                        let inner = &cell[after + 1..after + close_paren_rel];
                        let url = inner.split_whitespace().next().unwrap_or("");
                        if !url.is_empty() {
                            links.push(url.to_string());
                        }
                        i = after + close_paren_rel + 1;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
    links
}

/// Collapse `.`/`..` without requiring the path to exist. Identical shape
/// to `doc_links::normalise`.
fn normalise(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Core comparison, pure in its inputs for testing with synthetic fixtures.
/// `mapping_dir` is the directory Evidence-column links are resolved
/// against.
#[cfg(test)]
pub fn run_check(src: &str, mapping_dir: &str) -> ExitCode {
    run_check_with(src, mapping_dir, crate::citation::TEST_REPOSITORY_URL)
}

/// As [`run_check`], with the repository URL an absolute citation may use
/// (RFC-0.33-004 D4): a link under it is checked as the repo-relative path it names.
pub fn run_check_with(src: &str, mapping_dir: &str, repository_url: &str) -> ExitCode {
    use crate::citation::{Citation, classify};
    let rows = parse_rows(src);
    let mut problems: Vec<String> = Vec::new();
    let mut checked = 0usize;

    for row in &rows {
        match row {
            MappingRow::Malformed {
                line,
                label,
                reason,
            } => {
                problems.push(format!(
                    "line {line} ({label:?}): unreadable row — {reason}"
                ));
            }
            MappingRow::Row {
                line,
                id,
                status,
                evidence,
            } => {
                if !STATUS_VALUES.contains(&status.as_str()) {
                    problems.push(format!(
                        "{id} (line {line}): status {status:?} is not in the closed vocabulary {STATUS_VALUES:?}"
                    ));
                    continue;
                }
                if id.starts_with(STRUCTURAL_ID_PREFIX) && status != "unassessed" {
                    problems.push(format!(
                        "{id} (line {line}): structural row (prefix {STRUCTURAL_ID_PREFIX:?}) carries status {status:?} — a row whose criterion has not been read may only be `unassessed`"
                    ));
                    continue;
                }
                checked += 1;
                let paths = extract_evidence_paths(evidence);
                if matches!(status.as_str(), "met" | "partial" | "unassessed") && paths.is_empty() {
                    problems.push(format!(
                        "{id} (line {line}): status {status:?} requires at least one cited evidence path, found none"
                    ));
                }
                for p in &paths {
                    let resolved = match classify(p, repository_url) {
                        Citation::Relative(rel) => normalise(&Path::new(mapping_dir).join(rel)),
                        // From the repository root, not the mapping's directory.
                        Citation::Repository(path) => normalise(Path::new(&path)),
                        Citation::Foreign(url) => {
                            problems.push(format!(
                                "{id} (line {line}): cited URL {url:?} is not a citation into this \
                                 repository — an absolute citation must be \
                                 `{repository_url}/blob/main/<path>` so it can be checked against the tree"
                            ));
                            continue;
                        }
                    };
                    if !resolved.exists() {
                        problems.push(format!(
                            "{id} (line {line}): cited path {p:?} does not exist (resolves to {})",
                            resolved.display()
                        ));
                    }
                }
            }
        }
    }

    if problems.is_empty() {
        println!("standards-mapping: PASS ({checked} rows checked)");
        ExitCode::SUCCESS
    } else {
        eprintln!("standards-mapping: FAIL");
        for p in &problems {
            eprintln!("  {p}");
        }
        ExitCode::FAILURE
    }
}

pub fn check() -> ExitCode {
    let Some(src) = read_file("standards-mapping", MAPPING_PATH) else {
        return ExitCode::FAILURE;
    };
    let repository_url = match crate::citation::repository_url() {
        Ok(u) => u,
        Err(e) => {
            eprintln!("standards-mapping: FAIL — {e}");
            return ExitCode::FAILURE;
        }
    };
    run_check_with(&src, MAPPING_DIR, &repository_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIR: &str = "docs/src/compliance";

    #[test]
    fn parses_a_well_formed_row() {
        let src = "| CRA-I-1 | thing | met | mechanism | [x](../security/threat-model-v1.md) |";
        let rows = parse_rows(src);
        assert_eq!(
            rows,
            vec![MappingRow::Row {
                line: 1,
                id: "CRA-I-1".into(),
                status: "met".into(),
                evidence: "[x](../security/threat-model-v1.md)".into(),
            }]
        );
    }

    #[test]
    fn skips_header_and_separator_rows() {
        let src = "| ID | Requirement | Status | Mechanism | Evidence |\n\
                   |----|-------------|--------|-----------|----------|\n\
                   | CRA-I-1 | thing | not-applicable | m | — |";
        assert_eq!(parse_rows(src).len(), 1);
    }

    #[test]
    fn well_formed_not_applicable_row_with_no_evidence_passes() {
        let src = "| CRA-I-1 | thing | not-applicable | m | — |";
        assert_eq!(run_check(src, DIR), ExitCode::SUCCESS);
    }

    /// Required demonstration 1: an invented status value.
    #[test]
    fn invented_status_value_fails_naming_row_and_value() {
        let src = "| CRA-I-1 | thing | mostly-met | m | [x](../security/threat-model-v1.md) |";
        assert_eq!(run_check(src, DIR), ExitCode::FAILURE);
        match &parse_rows(src)[0] {
            MappingRow::Row { status, .. } => assert_eq!(status, "mostly-met"),
            other => panic!("expected a well-formed row with a bad status, got {other:?}"),
        }
    }

    /// Required demonstration 2: a `met` row with no cited path.
    #[test]
    fn met_row_with_no_path_fails() {
        let src = "| CRA-I-1 | thing | met | m | — |";
        assert_eq!(run_check(src, DIR), ExitCode::FAILURE);
    }

    /// Required demonstration 3: a row citing a deleted file.
    #[test]
    fn row_citing_a_nonexistent_file_fails() {
        let src = "| CRA-I-1 | thing | met | m | [x](../does-not-exist.md) |";
        assert_eq!(run_check(src, DIR), ExitCode::FAILURE);
    }

    /// Required demonstration 4: a row with an empty status cell must FAIL,
    /// not be silently skipped — the row must still be counted as
    /// unreadable, not dropped from the table.
    #[test]
    fn empty_status_cell_fails_and_is_not_skipped() {
        let src = "| CRA-I-1 | thing |  | m | — |";
        let rows = parse_rows(src);
        assert_eq!(rows.len(), 1, "the row must still be reported, not dropped");
        assert!(matches!(rows[0], MappingRow::Malformed { .. }));
        assert_eq!(run_check(src, DIR), ExitCode::FAILURE);
    }

    #[test]
    fn wrong_column_count_fails_and_is_not_skipped() {
        let src = "| CRA-I-1 | thing | met |";
        let rows = parse_rows(src);
        assert_eq!(rows.len(), 1);
        assert!(matches!(rows[0], MappingRow::Malformed { .. }));
        assert_eq!(run_check(src, DIR), ExitCode::FAILURE);
    }

    #[test]
    fn partial_row_requires_a_path_same_as_met() {
        let src = "| CRA-I-1 | thing | partial | m | — |";
        assert_eq!(run_check(src, DIR), ExitCode::FAILURE);
    }

    #[test]
    fn not_met_row_with_a_path_is_still_checked_for_existence() {
        let src = "| CRA-I-1 | thing | not-met | m | [x](../does-not-exist.md) |";
        assert_eq!(
            run_check(src, DIR),
            ExitCode::FAILURE,
            "direction B: any cited path must exist, regardless of status"
        );
    }

    /// Required demonstration 5 (added on review of `fb05a1a`): a
    /// structural (IEC) row set to `met` must FAIL — a row whose criterion
    /// has not been read may only ever be `unassessed`.
    #[test]
    fn iec_row_carrying_a_verdict_fails() {
        let src = "| IEC-4-1-SR | thing | met | m | [x](../security/threat-model-v1.md) |";
        assert_eq!(run_check(src, DIR), ExitCode::FAILURE);
    }

    #[test]
    fn iec_row_at_unassessed_with_a_path_passes() {
        // `cargo test` runs with cwd = this crate's own directory, not the
        // workspace root — `Cargo.toml` here is guaranteed to exist under
        // that cwd, unlike a path into `docs/`.
        let src = "| IEC-4-1-SR | thing | unassessed | m | [x](Cargo.toml) |";
        assert_eq!(run_check(src, "."), ExitCode::SUCCESS);
    }

    #[test]
    fn unassessed_row_requires_a_path_same_as_met_and_partial() {
        let src = "| IEC-4-1-SR | thing | unassessed | m | — |";
        assert_eq!(run_check(src, DIR), ExitCode::FAILURE);
    }

    #[test]
    fn non_iec_row_may_carry_any_closed_vocabulary_status() {
        let src = "| CRA-I-1 | thing | not-applicable | m | — |";
        assert_eq!(run_check(src, DIR), ExitCode::SUCCESS);
    }

    #[test]
    fn extracts_multiple_evidence_paths() {
        let cell = "[a](../security/threat-model-v1.md); [b](../release/v1-limitations.md)";
        assert_eq!(
            extract_evidence_paths(cell),
            vec![
                "../security/threat-model-v1.md".to_string(),
                "../release/v1-limitations.md".to_string(),
            ]
        );
    }

    // ── RFC-0.33-004 D4 / E-052: an absolute repository URL is a citation ────

    const URL: &str = "https://example.test/org/repo";

    /// `cargo test` runs with cwd = this crate's directory, so `Cargo.toml` is a
    /// file that exists at the "repository root" these tests resolve against.
    #[test]
    fn a_repository_url_to_an_existing_file_is_accepted() {
        let src =
            format!("| IEC-4-1-SR | thing | unassessed | m | [x]({URL}/blob/main/Cargo.toml) |");
        assert_eq!(run_check(&src, DIR), ExitCode::SUCCESS);
    }

    /// The other half of the demonstration: a citation that resolves nowhere is
    /// **still refused**, in URL form too.
    #[test]
    fn a_repository_url_to_a_missing_file_is_still_refused() {
        let src = format!(
            "| IEC-4-1-SR | thing | unassessed | m | [x]({URL}/blob/main/no/such/file.md) |"
        );
        assert_eq!(run_check(&src, DIR), ExitCode::FAILURE);
    }

    #[test]
    fn a_url_outside_the_repository_is_refused_not_silently_passed() {
        let src =
            "| IEC-4-1-SR | thing | unassessed | m | [x](https://elsewhere.test/Cargo.toml) |";
        assert_eq!(run_check(src, DIR), ExitCode::FAILURE);
        let src =
            format!("| IEC-4-1-SR | thing | unassessed | m | [x]({URL}/blob/abc123/Cargo.toml) |");
        assert_eq!(
            run_check(&src, DIR),
            ExitCode::FAILURE,
            "a pinned commit is not the default branch"
        );
    }

    #[test]
    fn a_relative_citation_still_works() {
        let src = "| IEC-4-1-SR | thing | unassessed | m | [x](Cargo.toml) |";
        assert_eq!(run_check(src, "."), ExitCode::SUCCESS);
    }

    /// The published table: every citation in the real file is a URL or an
    /// in-book path, none leaves the book by relative path, and the whole thing
    /// passes against the real tree.
    #[test]
    fn the_published_mapping_cites_nothing_by_a_path_that_leaves_the_book() {
        let src = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../")
                .join(MAPPING_PATH),
        )
        .unwrap();
        assert!(
            !src.contains("](../../../"),
            "a citation still leaves the book by relative path (E-052)"
        );
    }
}
