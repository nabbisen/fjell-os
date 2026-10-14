//! `readiness-summary` — `v1-readiness.md`'s own "Summary at …" block must
//! agree with the rows it summarises (RFC-0.35-003 R5).
//!
//! Gate 5 (`fjell-readiness-check`) only ever checks `open == 0` — it reads
//! the matrix, counts DONE/IN PROGRESS/DEFERRED/OPEN cells, prints them,
//! and passes on zero OPEN. **It never compares those counts with the
//! document's own "Summary at …" table or prose.** Twice in two days
//! (2026-10-13, 2026-10-14) a human had to notice the block disagreeing
//! with itself and correct it by hand, with Gate 5 green throughout — the
//! defect this subcheck exists to close.
//!
//! **Row counting duplicates `fjell-readiness-check::check_matrix`**
//! deliberately, not by oversight — the same convention
//! `capability_topology`'s own module doc states: these tools do not
//! depend on each other, and the function is a dozen lines.
//!
//! **What this checks:**
//! 1. The summary table's four counts (DONE/IN PROGRESS/DEFERRED/OPEN)
//!    equal what counting every `|`-prefixed row in the document actually
//!    produces.
//! 2. The prose immediately below the table — the one paragraph before the
//!    first correction blockquote, so a historical `>` note's own figures
//!    (which may legitimately *name* an old, wrong number) are never read
//!    as a live claim — states no number-word for "OPEN cells" or "IN
//!    PROGRESS" that the table itself contradicts.
//! 3. (R6) The heading's own version (`## Summary at <X>`) equals the
//!    current workspace version (`Cargo.toml`'s `[workspace.package]
//!    version`) — enforced, not merely noted: the heading is a factual
//!    claim about *when* these counts were taken, and both prior
//!    incidents included a stale heading. A line that touches no row of
//!    this matrix still leaves the heading true, because the workspace
//!    version does not change until a cut, and the counts do not change
//!    unless a row does — so this is never a false alarm mid-milestone,
//!    only at the boundary the two incidents actually occurred on.

use crate::read_file;
use std::process::ExitCode;

const NAME: &str = "readiness-summary";
const MATRIX_PATH: &str = "docs/src/releasing/v1-readiness.md";
const CARGO_TOML_PATH: &str = "Cargo.toml";

#[derive(Debug, PartialEq, Eq, Default)]
pub struct RowCounts {
    pub done: usize,
    pub in_progress: usize,
    pub deferred: usize,
    pub open: usize,
}

pub fn check() -> ExitCode {
    let Some(matrix_src) = read_file(NAME, MATRIX_PATH) else {
        return ExitCode::FAILURE;
    };
    let Some(cargo_src) = read_file(NAME, CARGO_TOML_PATH) else {
        return ExitCode::FAILURE;
    };
    let Some(current_version) = crate::version_currency::parse_workspace_version(&cargo_src) else {
        eprintln!("{NAME}: FAIL — cannot find workspace version in {CARGO_TOML_PATH}");
        return ExitCode::FAILURE;
    };
    run_check(&matrix_src, &current_version)
}

/// Every `|`-prefixed table row in the whole document, classified the same
/// way `fjell-readiness-check::check_matrix` does — this includes the
/// summary's own table (it is `|`-prefixed too), which is why the actual
/// count this compares against is taken from the **dimension** tables only
/// (everything before `## Summary at`), not the whole file.
fn count_dimension_rows(src: &str) -> RowCounts {
    let dimension_src = match src.find("\n## Summary at") {
        Some(idx) => &src[..idx],
        None => src,
    };
    let mut c = RowCounts::default();
    for line in dimension_src.lines() {
        if !line.trim_start().starts_with('|') {
            continue;
        }
        if line.contains("**DONE**") || line.contains("DONE (") {
            c.done += 1;
        } else if line.contains("**IN PROGRESS**") || line.contains("IN_PROGRESS") {
            c.in_progress += 1;
        } else if line.contains("**DEFERRED**") {
            c.deferred += 1;
        } else if line.contains("**OPEN**") {
            c.open += 1;
        }
    }
    c
}

/// The `## Summary at <X>` block: the heading itself, through the first
/// correction blockquote (exclusive) or end of file if there is none yet.
/// Scoped so a historical `>` note's own quoted figures — which may
/// legitimately *name* an old, wrong number as part of explaining it — are
/// never read as this document's live claim.
fn summary_block(src: &str) -> Option<&str> {
    let start = src.find("## Summary at")?;
    let rest = &src[start..];
    let end = rest.find("\n>").unwrap_or(rest.len());
    Some(&rest[..end])
}

fn parse_heading_version(block: &str) -> Option<&str> {
    let line = block.lines().next()?;
    line.strip_prefix("## Summary at ").map(str::trim)
}

fn parse_table_count(block: &str, label: &str) -> Option<usize> {
    let prefix = format!("| {label} |");
    let line = block
        .lines()
        .find(|l| l.trim_start().starts_with(&prefix))?;
    let rest = line.trim_start().strip_prefix(&prefix)?;
    let rest = rest.trim_start().trim_start_matches('|').trim();
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

fn parse_table_counts(block: &str) -> RowCounts {
    RowCounts {
        done: parse_table_count(block, "DONE").unwrap_or(0),
        in_progress: parse_table_count(block, "IN PROGRESS").unwrap_or(0),
        deferred: parse_table_count(block, "DEFERRED").unwrap_or(0),
        open: parse_table_count(block, "OPEN").unwrap_or(0),
    }
}

/// `Zero`/`One`/… `Ten`, case-insensitive — the number words this
/// document's own prose actually uses. Bounded deliberately: a readiness
/// matrix with more than ten rows of one status would need a new word this
/// project does not use today, and a parser that silently accepted
/// "eleven" without a test proving it reads that word correctly would be
/// worse than one that visibly does not try.
fn number_word(w: &str) -> Option<usize> {
    const WORDS: &[&str] = &[
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
    ];
    WORDS.iter().position(|w2| w.eq_ignore_ascii_case(w2))
}

/// Everything after the table's own `| OPEN | N |` row — the prose
/// paragraph the table is followed by, with none of the table's own row
/// labels (which also contain the literal text "IN PROGRESS") in scope.
fn prose_after_table<'a>(block: &'a str) -> &'a str {
    match block.find("| OPEN |") {
        Some(pos) => {
            let after = &block[pos..];
            match after.find('\n') {
                Some(nl) => &after[nl + 1..],
                None => "",
            }
        }
        None => block,
    }
}

/// The number word immediately before the first occurrence of `needle` in
/// `text`, if any.
fn word_before(text: &str, needle: &str) -> Option<usize> {
    let pos = text.find(needle)?;
    let before = text[..pos].trim_end();
    let word = before
        .rsplit(|c: char| c.is_whitespace() || c == '*')
        .next()?;
    number_word(word)
}

/// Core comparison, pure in its inputs for testing with a fixture.
pub fn run_check(matrix_src: &str, current_version: &str) -> ExitCode {
    let Some(block) = summary_block(matrix_src) else {
        eprintln!("{NAME}: FAIL — no `## Summary at` heading found in {MATRIX_PATH}");
        return ExitCode::FAILURE;
    };

    let mut problems = Vec::new();

    let actual = count_dimension_rows(matrix_src);
    let claimed = parse_table_counts(block);
    for (label, c, a) in [
        ("DONE", claimed.done, actual.done),
        ("IN PROGRESS", claimed.in_progress, actual.in_progress),
        ("DEFERRED", claimed.deferred, actual.deferred),
        ("OPEN", claimed.open, actual.open),
    ] {
        if c != a {
            problems.push(format!(
                "summary table claims {label} {c}; the dimension rows count {a}"
            ));
        }
    }

    // Prose only, not the table above it: the table's own "| IN PROGRESS |"
    // row label would otherwise be the first "IN PROGRESS" match, with no
    // number word in front of it, hiding a real contradiction in the prose
    // that follows.
    let prose = prose_after_table(block);
    if let Some(prose_open) = word_before(prose, "OPEN cell") {
        if prose_open != claimed.open {
            problems.push(format!(
                "prose says {prose_open} OPEN cell(s); the summary table itself says {}",
                claimed.open
            ));
        }
    }
    if let Some(prose_ip) = word_before(prose, "IN PROGRESS") {
        if prose_ip != claimed.in_progress {
            problems.push(format!(
                "prose says {prose_ip} IN PROGRESS row(s); the summary table itself says {}",
                claimed.in_progress
            ));
        }
    }

    match parse_heading_version(block) {
        Some(v) if v == current_version => {}
        Some(v) => problems.push(format!(
            "heading says \"Summary at {v}\"; the workspace version is {current_version}"
        )),
        None => problems.push("could not parse a version from the `## Summary at` heading".into()),
    }

    if problems.is_empty() {
        println!("{NAME}: PASS (summary agrees with {MATRIX_PATH}'s own rows)");
        ExitCode::SUCCESS
    } else {
        eprintln!("{NAME}: FAIL");
        for p in &problems {
            eprintln!("  {p}");
        }
        ExitCode::FAILURE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dimension_rows(done: usize, in_progress: usize, deferred: usize, open: usize) -> String {
        let mut s = String::from("| Item | RFC | Status |\n|---|---|---|\n");
        for _ in 0..done {
            s.push_str("| x | y | **DONE** (v0.1.0) |\n");
        }
        for _ in 0..in_progress {
            s.push_str("| x | y | **IN PROGRESS** → v1.x |\n");
        }
        for _ in 0..deferred {
            s.push_str("| x | y | **DEFERRED** — post-v1.0 |\n");
        }
        for _ in 0..open {
            s.push_str("| x | y | **OPEN** |\n");
        }
        s
    }

    fn current_block(version: &str, done: usize, in_progress: usize) -> String {
        format!(
            "\n## Summary at {version}\n\n\
             | Status | Count |\n|--------|-------|\n\
             | DONE | {done} |\n| IN PROGRESS | {in_progress} |\n| DEFERRED | 3 |\n| OPEN | 0 |\n\n\
             *Zero OPEN cells. {ip_word} IN PROGRESS rows — both `→ v1.x`.*\n",
            ip_word = match in_progress {
                0 => "Zero",
                1 => "One",
                2 => "Two",
                _ => "Several",
            }
        )
    }

    #[test]
    fn matching_block_passes() {
        let src = format!(
            "{}{}",
            dimension_rows(56, 2, 3, 0),
            current_block("0.34.0", 56, 2)
        );
        assert_eq!(run_check(&src, "0.34.0"), ExitCode::SUCCESS);
    }

    /// R5's own required demonstration: 2026-10-12's actual disagreement,
    /// reconstructed — headed "at v0.9.4", claiming DONE 42 where the rows
    /// count 56, and prose claiming three IN PROGRESS where the table
    /// itself says 0 and the rows number 2.
    #[test]
    fn the_2026_10_12_block_fails() {
        let rows = dimension_rows(56, 2, 3, 0);
        let stale_block = "\n## Summary at v0.9.4\n\n\
             | Status | Count |\n|--------|-------|\n\
             | DONE | 42 |\n| IN PROGRESS | 0 |\n| DEFERRED | 3 |\n| OPEN | 0 |\n\n\
             *Zero OPEN cells. Three IN PROGRESS items.*\n";
        let src = format!("{rows}{stale_block}");
        assert_eq!(run_check(&src, "0.34.0"), ExitCode::FAILURE);
    }

    #[test]
    fn drifted_done_count_fails() {
        let src = format!(
            "{}{}",
            dimension_rows(57, 2, 3, 0), // one more DONE row than the summary claims
            current_block("0.34.0", 56, 2)
        );
        assert_eq!(run_check(&src, "0.34.0"), ExitCode::FAILURE);
    }

    #[test]
    fn prose_contradicting_its_own_table_fails_even_with_matching_counts() {
        // The table is internally consistent with the rows; only the prose
        // lies about it — R5's second clause, independent of the first.
        let rows = dimension_rows(56, 2, 3, 0);
        let block = "\n## Summary at 0.34.0\n\n\
             | Status | Count |\n|--------|-------|\n\
             | DONE | 56 |\n| IN PROGRESS | 2 |\n| DEFERRED | 3 |\n| OPEN | 0 |\n\n\
             *Zero OPEN cells. Three IN PROGRESS rows.*\n";
        let src = format!("{rows}{block}");
        assert_eq!(run_check(&src, "0.34.0"), ExitCode::FAILURE);
    }

    #[test]
    fn stale_heading_version_fails() {
        let src = format!(
            "{}{}",
            dimension_rows(56, 2, 3, 0),
            current_block("0.33.0", 56, 2) // 0.34.0 shipped; heading never moved
        );
        assert_eq!(run_check(&src, "0.34.0"), ExitCode::FAILURE);
    }

    #[test]
    fn a_correction_blockquote_s_own_figures_are_not_read_as_a_live_claim() {
        // The blockquote names old, wrong numbers as part of explaining the
        // history (exactly as the real document's own correction notes
        // do) -- summary_block must stop before it, or this historical
        // quoting would itself trip the comparison.
        let rows = dimension_rows(56, 2, 3, 0);
        let block = format!(
            "{}\n> Corrected: this used to say DONE 42 and three IN PROGRESS cells where the table said 0.\n",
            current_block("0.34.0", 56, 2)
        );
        let src = format!("{rows}{block}");
        assert_eq!(run_check(&src, "0.34.0"), ExitCode::SUCCESS);
    }

    #[test]
    fn number_word_parses_the_bounded_set() {
        assert_eq!(number_word("Zero"), Some(0));
        assert_eq!(number_word("two"), Some(2));
        assert_eq!(number_word("TEN"), Some(10));
        assert_eq!(number_word("eleven"), None);
        assert_eq!(number_word("56"), None);
    }

    #[test]
    fn count_dimension_rows_excludes_the_summary_table_itself() {
        // The summary table's own rows are `|`-prefixed and would
        // over-count if dimension counting did not stop before them.
        let rows = dimension_rows(2, 0, 0, 0);
        let src = format!("{}{}", rows, current_block("0.34.0", 2, 0));
        let c = count_dimension_rows(&src);
        assert_eq!(
            c.done, 2,
            "must not also count the summary table's own DONE row"
        );
    }
}
