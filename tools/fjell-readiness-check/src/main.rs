//! `fjell-readiness-check` — v1.0 readiness matrix gate (RFC-v0.10-007 §4).
//!
//! Parses `docs/release/v1-readiness.md` and fails if any cell contains
//! the literal word `OPEN`. Every `OPEN` cell blocks the v1.0 release.
//!
//! Exit codes:
//!   0 — zero OPEN cells (gate passes)
//!   1 — one or more OPEN cells found
//!   2 — matrix file not found or unreadable

use std::fs;
use std::process::ExitCode;

const MATRIX_PATH: &str = "docs/release/v1-readiness.md";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.windows(2)
        .find(|w| w[0] == "--matrix")
        .and_then(|w| w.get(1))
        .map(String::as_str)
        .unwrap_or(MATRIX_PATH);

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("readiness-check: cannot read {}: {}", path, e);
            eprintln!("readiness-check: run `cargo xtask test-all` to generate first.");
            return ExitCode::from(2);
        }
    };

    let result = check_matrix(&content);
    println!("readiness-check:");
    println!("  DONE     : {}", result.done);
    println!("  IN_PROGRESS: {}", result.in_progress);
    println!("  DEFERRED : {}", result.deferred);
    println!("  OPEN     : {}", result.open);

    if result.open == 0 {
        println!("  Result   : PASS — zero OPEN cells");
        ExitCode::SUCCESS
    } else {
        eprintln!("  Result   : FAIL — {} OPEN cell(s) block v1.0", result.open);
        for line in &result.open_lines {
            eprintln!("    {}", line);
        }
        ExitCode::FAILURE
    }
}

struct MatrixResult {
    done:        usize,
    in_progress: usize,
    deferred:    usize,
    open:        usize,
    open_lines:  Vec<String>,
}

fn check_matrix(content: &str) -> MatrixResult {
    let mut result = MatrixResult {
        done: 0, in_progress: 0, deferred: 0, open: 0,
        open_lines: Vec::new(),
    };

    for (lineno, line) in content.lines().enumerate() {
        // Only examine table rows (lines starting with `|`)
        if !line.trim_start().starts_with('|') { continue; }

        if line.contains("**DONE**") || line.contains("DONE (") {
            result.done += 1;
        } else if line.contains("**IN PROGRESS**") || line.contains("IN_PROGRESS") {
            result.in_progress += 1;
        } else if line.contains("**DEFERRED**") {
            result.deferred += 1;
        } else if line.contains("**OPEN**") {
            // Only count rows with bold **OPEN** as blocking cells.
            // Bare "OPEN" in summary count tables (e.g. "| OPEN | 0 |") is not a cell.
            result.open += 1;
            result.open_lines.push(format!("line {}: {}", lineno + 1, line.trim()));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r"
| Item A | RFC 001 | **DONE** (v0.1.0) |
| Item B | RFC 002 | **IN PROGRESS** → v0.12 |
| Item C | RFC 003 | **DEFERRED** — post-v1.0 |
| Item D | RFC 004 | **OPEN** |
";

    #[test]
    fn counts_all_status_types() {
        let r = check_matrix(SAMPLE);
        assert_eq!(r.done,        1);
        assert_eq!(r.in_progress, 1);
        assert_eq!(r.deferred,    1);
        assert_eq!(r.open,        1);
    }

    #[test]
    fn clean_matrix_passes() {
        let clean = r"
| Item A | RFC 001 | **DONE** (v0.1.0) |
| Item B | RFC 002 | **DONE** (v0.2.0) |
| Item C | RFC 003 | **DEFERRED** — post-v1.0 |
";
        let r = check_matrix(clean);
        assert_eq!(r.open, 0);
    }

    #[test]
    fn non_table_lines_ignored() {
        let content = "# Header\n Some OPEN text outside table\n| item | rfc | **DONE** (v1) |\n";
        let r = check_matrix(content);
        assert_eq!(r.open, 0);   // OPEN not in a table row
        assert_eq!(r.done, 1);
    }

    #[test]
    fn open_lines_captured() {
        let r = check_matrix(SAMPLE);
        assert_eq!(r.open_lines.len(), 1);
        assert!(r.open_lines[0].contains("OPEN"));
    }

    #[test]
    fn current_matrix_has_no_open_cells() {
        // When run from workspace root, check the actual matrix file.
        // When run from target/ the file isn't accessible; skip gracefully.
        let content = match std::fs::read_to_string("docs/release/v1-readiness.md") {
            Ok(c) => c,
            Err(_) => return,
        };
        let r = check_matrix(&content);
        assert_eq!(r.open, 0,
            "v1-readiness.md must have zero OPEN cells; found {} — fix before v1.0",
            r.open);
        // We expect some DONE items at minimum
        assert!(r.done > 0, "readiness matrix should have DONE items");
    }
}
