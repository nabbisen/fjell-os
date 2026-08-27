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
//!   - `syscall-surface` — declared vs. dispatched syscalls vs. the
//!     committed expectations file (Slice 1)
//!   - `errata-limitations` — every ACCEPTED erratum is referenced in the
//!     v1.0 limitations doc (Slice 4)
//!   - `rfc-status-folder` — each RFC's Status agrees with its folder
//!     (Slice 4)
//!   - `handoff-status` — each handoff's inherited Status matches its
//!     governing RFC (Slice 4)
//!
//! Usage:
//!   `fjell-consistency-check <subcheck>`
//!   `fjell-consistency-check --all`   — runs every subcheck (this is Gate 12)

use std::fs;
use std::process::ExitCode;

mod doc_counts;
mod doc_links;
mod errata_limitations;
mod errata_tracking;
mod handoff_status;
mod rfc_status_folder;
mod status;
mod syscall_surface;
mod version_currency;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sub = args.first().map(String::as_str).unwrap_or("--help");

    match sub {
        "syscall-surface" => run_named("syscall-surface", syscall_surface::check),
        "errata-limitations" => run_named("errata-limitations", errata_limitations::check),
        "rfc-status-folder" => run_named("rfc-status-folder", rfc_status_folder::check),
        "handoff-status" => run_named("handoff-status", handoff_status::check),
        "errata-tracking" => run_named("errata-tracking", errata_tracking::check),
        "version-currency" => run_named("version-currency", version_currency::check),
        "doc-links" => run_named("doc-links", doc_links::check),
        "doc-counts" => run_named("doc-counts", doc_counts::check),
        "--all" => run_all(),
        _ => {
            eprintln!(
                "Usage: fjell-consistency-check \
                 <syscall-surface|errata-limitations|rfc-status-folder|handoff-status|\
                 errata-tracking|version-currency|doc-links|doc-counts|--all>"
            );
            ExitCode::FAILURE
        }
    }
}

type Subcheck = (&'static str, fn() -> ExitCode);

/// All subchecks, in the order Gate 12 reports them.
const ALL_SUBCHECKS: &[Subcheck] = &[
    ("syscall-surface", syscall_surface::check),
    ("errata-limitations", errata_limitations::check),
    ("rfc-status-folder", rfc_status_folder::check),
    ("handoff-status", handoff_status::check),
    ("errata-tracking", errata_tracking::check),
    ("version-currency", version_currency::check),
    ("doc-links", doc_links::check),
    ("doc-counts", doc_counts::check),
];

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
