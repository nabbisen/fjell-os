//! QEMU negative test runner for
//! `cargo xtask qemu-negative <category>`.
//!
//! Per RFC 026 (negative-test harness) and RFC 042 (v0.2 expansion),
//! every category corresponds to a profile under
//! `tests/qemu/profiles/<category>.toml`. A category with no profile is
//! an error (RFC-0.24-002 Slice 4) — the RFC 025 placeholder path that
//! silently passed against an empty expectation set (`lease`, `evidence`
//! reachable with no profile ever written for either) was removed, not
//! bypassed, since a category `KNOWN_*` lists but never wires up a
//! profile for is itself the defect, not a state to run cleanly through.

use std::path::Path;
use std::process::ExitCode;

const KNOWN_V01X_CATEGORIES: &[&str] = &[
    "capability",
    "cap", // "cap" is an accepted alias for "capability"
    "ipc",
    "mmio",
    "dma",
    "store",
    "upgrade",
];

const KNOWN_V02_CATEGORIES: &[&str] = &["lease", "user-copy", "audit", "policy", "evidence", "svc"];

/// Entry point: `cargo xtask qemu-negative <category>`.
pub fn cmd_qemu_negative(category: Option<&str>) -> ExitCode {
    let category = match category {
        Some(c) => c,
        None => {
            eprintln!("Usage: cargo xtask qemu-negative <category>");
            eprintln!(
                "Known categories (v0.1.x): {}",
                KNOWN_V01X_CATEGORIES.join(", ")
            );
            eprintln!(
                "Reserved for v0.2:         {}",
                KNOWN_V02_CATEGORIES.join(", ")
            );
            return ExitCode::FAILURE;
        }
    };

    let profile_path = format!("tests/qemu/profiles/{category}.toml");
    if Path::new(&profile_path).exists() {
        // Delegate to the explicit loader via qemu_run::cmd_qemu_run.
        return crate::qemu_run::cmd_qemu_run(Some(category));
    }

    if !KNOWN_V01X_CATEGORIES.contains(&category) && !KNOWN_V02_CATEGORIES.contains(&category) {
        eprintln!("[xtask] qemu-negative: unknown category `{category}`");
    } else {
        eprintln!(
            "[xtask] qemu-negative: `{category}` is a known category with no \
             profile at {profile_path} — write one before running it."
        );
    }
    ExitCode::FAILURE
}
