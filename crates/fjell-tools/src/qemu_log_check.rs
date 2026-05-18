//! `cargo xtask qemu-log-check <log-file> <marker>` — generic marker
//! validator.
//!
//! Used by every CI job to assert that a captured serial log contains a
//! given `TEST:Mx:PASS` or `NEG:CATEGORY:CASE:PASS` marker. Kept
//! deliberately tiny — all parsing is line-by-line contains-check.
//!
//! Per RFC 025: this tool is the only validator allowed for the smoke
//! and negative-test paths. No per-test custom matcher is used.

use std::process::ExitCode;
use std::path::Path;

/// Entry point: `cargo xtask qemu-log-check <log-file> <marker>`.
pub fn cmd_qemu_log_check(log: Option<&str>, marker: Option<&str>) -> ExitCode {
    let log = match log {
        Some(p) => p,
        None => {
            eprintln!("Usage: cargo xtask qemu-log-check <log-file> <marker>");
            return ExitCode::FAILURE;
        }
    };
    let marker = match marker {
        Some(m) => m,
        None => {
            eprintln!("Usage: cargo xtask qemu-log-check <log-file> <marker>");
            return ExitCode::FAILURE;
        }
    };
    log_check(Path::new(log), marker)
}

/// Read the file and search for the marker.  Returns SUCCESS only if
/// the marker appears as a substring of any line.
///
/// Exposed as a library function so smoke and negative runners can
/// reuse it without going through a subprocess.
pub fn log_check(log_path: &Path, marker: &str) -> ExitCode {
    let content = match std::fs::read_to_string(log_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[xtask] qemu-log-check: cannot read {}: {e}",
                      log_path.display());
            return ExitCode::FAILURE;
        }
    };
    if content.lines().any(|l| l.contains(marker)) {
        println!("[xtask] FOUND `{marker}` in {} ✓", log_path.display());
        ExitCode::SUCCESS
    } else {
        eprintln!("[xtask] `{marker}` NOT found in {}", log_path.display());
        ExitCode::FAILURE
    }
}
