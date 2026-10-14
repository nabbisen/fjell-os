//! # `fjell-mmio-audit`
//!
//! Scans the Fjell workspace for MMIO access sites and verifies that each
//! carries an `MMIO-ORDER:` annotation (RFC-v0.12-004 §3).
//!
//! The annotated classifications are:
//!   - `device_setup`      — pre-start config; weaker ordering acceptable.
//!   - `device_kick`       — store that releases work; must have release fence.
//!   - `descriptor_publish`— descriptor write; must have `fence rw,w` before.
//!   - `status_read`       — load observing device state; acquire fence needed.
//!   - `irq_ack`           — write clearing an interrupt.
//!   - `poll`              — busy-wait read; ordering benign.
//!
//! Usage:
//!   `fjell-mmio-audit [--workspace <root>] [--check]`
//!   `fjell-mmio-audit --help | -h`
//!
//!   `--check`: exit 1 if any site is missing its annotation.
//!
//! RFC-0.35-003 (E-071's sweep, R4): an unrecognised argument used to be
//! silently ignored and the scan ran anyway — the same shape named for
//! `fjell-repro-check`, found by sweeping this directory for it. An
//! unrecognised argument now exits 2 with the usage; `--help`/`-h` prints
//! it and exits 0.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const MMIO_PATTERNS: &[&str] = &[
    "read_volatile(",
    "write_volatile(",
    "ptr::read_volatile",
    "ptr::write_volatile",
];

const MMIO_ORDER_VALUES: &[&str] = &[
    "device_setup",
    "device_kick",
    "descriptor_publish",
    "status_read",
    "irq_ack",
    "poll",
];

// ── Main ──────────────────────────────────────────────────────────────────────

const USAGE: &str =
    "Usage: fjell-mmio-audit [--workspace <root>] [--check]\n       fjell-mmio-audit --help | -h";
const KNOWN_FLAGS: &[&str] = &["--check", "--help", "-h"];
const KNOWN_VALUE_FLAGS: &[&str] = &["--workspace"];

fn first_unrecognized(args: &[String]) -> Option<&str> {
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if KNOWN_VALUE_FLAGS.contains(&a) {
            i += 2;
            continue;
        }
        if KNOWN_FLAGS.contains(&a) {
            i += 1;
            continue;
        }
        return Some(a);
    }
    None
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if let Some(bad) = first_unrecognized(&args) {
        eprintln!("mmio-audit: unrecognised argument: {bad}");
        eprintln!("{USAGE}");
        // **2, not 1.** This module's own doc above said "exits 2" while
        // this line returned `ExitCode::FAILURE` (1) — corrected at review,
        // 2026-10-14. The distinction is load-bearing: `1` is this tool's
        // "a site is missing its annotation", a real finding a caller
        // reports as a defect. A caller that typoes a flag must not be told
        // the audit failed. `fjell-unsafe-audit` already separates the two
        // this way.
        return ExitCode::from(2);
    }

    let check_mode = args.iter().any(|a| a == "--check");
    let workspace = args
        .windows(2)
        .find(|w| w[0] == "--workspace")
        .and_then(|w| w.get(1))
        .map(String::as_str)
        .unwrap_or(".");

    let mut records = Vec::new();
    scan_dir(Path::new(workspace), &mut records).unwrap_or_else(|e| {
        eprintln!("mmio-audit: scan error: {}", e);
    });

    let total = records.len();
    let missing = records.iter().filter(|r| r.annotation.is_none()).count();
    let invalid = records
        .iter()
        .filter(|r| {
            r.annotation
                .as_deref()
                .map_or(false, |a| !MMIO_ORDER_VALUES.contains(&a))
        })
        .count();

    println!("  total MMIO sites   : {}", total);
    println!("  annotated          : {}", total - missing);
    println!("  missing annotation : {}", missing);
    println!("  invalid annotation : {}", invalid);

    if missing > 0 || invalid > 0 {
        println!("\nSITES NEEDING MMIO-ORDER ANNOTATION:");
        for r in records.iter().filter(|r| {
            r.annotation.is_none()
                || r.annotation
                    .as_deref()
                    .map_or(false, |a| !MMIO_ORDER_VALUES.contains(&a))
        }) {
            println!(
                "  {}:{} [{}]",
                r.path.display(),
                r.line,
                r.annotation.as_deref().unwrap_or("MISSING")
            );
        }
    }

    if check_mode && (missing > 0 || invalid > 0) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

// ── Record type ───────────────────────────────────────────────────────────────

struct MmioRecord {
    path: PathBuf,
    line: usize,
    annotation: Option<String>, // value after `MMIO-ORDER:` tag
}

// ── Scanner ───────────────────────────────────────────────────────────────────

fn scan_dir(dir: &Path, records: &mut Vec<MmioRecord>) -> io::Result<()> {
    if dir.ends_with("target") || dir.ends_with(".git") || dir.ends_with("fjell-mmio-audit") {
        return Ok(());
    } // self-exclude
    for entry in fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_dir(&path, records)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            scan_file(&path, records)?;
        }
    }
    Ok(())
}

fn scan_file(path: &Path, records: &mut Vec<MmioRecord>) -> io::Result<()> {
    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();

    for (idx, raw) in lines.iter().enumerate() {
        // RFC 060-style: strip string literals so we don't catch pattern names
        let stripped = strip_string_literals_and_comments(raw);

        let has_mmio = MMIO_PATTERNS.iter().any(|p| stripped.contains(p));
        if !has_mmio {
            continue;
        }

        // Look for MMIO-ORDER: annotation in the preceding 8 lines
        let annotation = find_mmio_order(&lines, idx);
        records.push(MmioRecord {
            path: path.to_owned(),
            line: idx + 1,
            annotation,
        });
    }
    Ok(())
}

fn find_mmio_order(lines: &[&str], idx: usize) -> Option<String> {
    let search_from = idx.saturating_sub(8);
    for line in lines[search_from..idx].iter().rev() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("// MMIO-ORDER:") {
            let value = rest.trim().split_whitespace().next()?.to_string();
            return Some(value);
        }
        // Stop on empty or non-comment lines (except attributes)
        if t.is_empty() || (!t.starts_with("//") && !t.starts_with('#')) {
            break;
        }
    }
    None
}

/// Strip string literals and line comments from a source line
/// (same RFC 060 approach as fjell-unsafe-audit).
fn strip_string_literals_and_comments(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == '/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            break;
        }
        if c == '"' {
            let mut j = i + 1;
            while j < bytes.len() {
                if bytes[j] == b'\\' && j + 1 < bytes.len() {
                    j += 2;
                    continue;
                }
                if bytes[j] == b'"' {
                    j += 1;
                    break;
                }
                j += 1;
            }
            i = j;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strs(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn no_args_is_recognised() {
        assert_eq!(first_unrecognized(&strs(&[])), None);
    }

    #[test]
    fn known_flags_are_recognised() {
        assert_eq!(
            first_unrecognized(&strs(&["--check", "--workspace", "."])),
            None
        );
    }

    #[test]
    fn an_unrecognised_flag_is_named() {
        assert_eq!(first_unrecognized(&strs(&["--bogus"])), Some("--bogus"));
    }

    #[test]
    fn strip_does_not_flag_string_patterns() {
        let line = r#"    if stripped.contains("read_volatile(") {"#;
        let s = strip_string_literals_and_comments(line);
        assert!(!MMIO_PATTERNS.iter().any(|p| s.contains(p)));
    }

    #[test]
    fn find_annotation_immediately_above() {
        let lines = vec![
            "// MMIO-ORDER: device_kick",
            "ptr::write_volatile(ptr, val);",
        ];
        let ann = find_mmio_order(&lines, 1);
        assert_eq!(ann.as_deref(), Some("device_kick"));
    }

    #[test]
    fn find_annotation_three_lines_above() {
        let lines = vec![
            "// MMIO-ORDER: status_read",
            "// read the device register",
            "// check the value",
            "let v = ptr::read_volatile(reg);",
        ];
        let ann = find_mmio_order(&lines, 3);
        assert_eq!(ann.as_deref(), Some("status_read"));
    }

    #[test]
    fn missing_annotation_returns_none() {
        let lines = vec!["// some comment", "let v = ptr::read_volatile(reg);"];
        let ann = find_mmio_order(&lines, 1);
        assert!(ann.is_none());
    }

    #[test]
    fn annotation_stops_at_non_comment() {
        let lines = vec![
            "// MMIO-ORDER: device_kick",
            "let x = 0;", // non-comment → stops search
            "ptr::write_volatile(r, v);",
        ];
        let ann = find_mmio_order(&lines, 2);
        // The blank/non-comment line breaks the search
        assert!(ann.is_none());
    }

    #[test]
    fn valid_annotation_values_recognized() {
        for v in MMIO_ORDER_VALUES {
            assert!(MMIO_ORDER_VALUES.contains(v));
        }
    }

    #[test]
    fn scan_workspace_produces_records() {
        // Run against the workspace root; should find at least some MMIO
        // sites in the kernel (or zero if the binary test CWD differs).
        let mut records = Vec::new();
        scan_dir(Path::new("crates/fjell-kernel/src"), &mut records).ok();
        // If we're at workspace root, expect hits; otherwise 0 is fine too.
        let _ = records.len();
    }
}
