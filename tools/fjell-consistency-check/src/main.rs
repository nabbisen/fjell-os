//! # `fjell-consistency-check`
//!
//! RFC-v0.22-001 (Gate Integrity): checks that the repository's *declared*
//! state agrees with its *actual* state. One tool, several independent
//! subchecks — Slices 1 and 4 of the RFC both answer the same question
//! ("does the repo say what it does?"), so they live here together rather
//! than as separate tools, per the architect's decision recorded in the
//! implementation handoff §0.1.
//!
//! Subchecks:
//!   - `syscall-surface`   — declared vs. dispatched syscalls vs. the
//!                           committed expectations file (Slice 1)
//!   - (Slice 4 adds `errata-limitations`, `rfc-status-folder`,
//!     `handoff-status`)
//!
//! Usage:
//!   `fjell-consistency-check <subcheck>`
//!   `fjell-consistency-check --all`   — runs every subcheck (this is Gate 12)

use std::fs;
use std::process::ExitCode;

mod syscall_surface;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sub = args.first().map(String::as_str).unwrap_or("--help");

    match sub {
        "syscall-surface" => run_named("syscall-surface", syscall_surface::check),
        "--all" => run_all(),
        _ => {
            eprintln!("Usage: fjell-consistency-check <syscall-surface|--all>");
            ExitCode::FAILURE
        }
    }
}

/// All subchecks, in the order Gate 12 reports them.
const ALL_SUBCHECKS: &[(&str, fn() -> ExitCode)] = &[("syscall-surface", syscall_surface::check)];

fn run_all() -> ExitCode {
    let mut all_ok = true;
    for (name, check) in ALL_SUBCHECKS {
        println!("--- consistency-check: {name} ---");
        if check() != ExitCode::SUCCESS {
            all_ok = false;
        }
    }
    if all_ok {
        println!(
            "consistency-check: PASS ({} subchecks)",
            ALL_SUBCHECKS.len()
        );
        ExitCode::SUCCESS
    } else {
        eprintln!("consistency-check: FAIL");
        ExitCode::FAILURE
    }
}

fn run_named(name: &str, check: fn() -> ExitCode) -> ExitCode {
    println!("--- consistency-check: {name} ---");
    check()
}

/// Read a file to a `String`, printing a consistent error and returning
/// `None` on failure so callers can report FAIL rather than panic.
pub(crate) fn read_file(path: &str) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!("consistency-check: cannot read {path}: {e}");
            None
        }
    }
}
