//! QEMU negative test runner for
//! `cargo xtask qemu-negative <category>`.
//!
//! Per RFC 026 (negative-test harness) and RFC 042 (v0.2 expansion),
//! every category corresponds to a profile under
//! `tests/qemu/profiles/<category>.toml`.  v0.1.1 ships only
//! placeholder profiles — they assert that the *infrastructure*
//! runs in CI without asserting any specific marker.  Real test
//! cases are added incrementally by v0.1.2 onwards (RFC 026 §case
//! bodies) and by each v0.2 RFC.

use std::process::ExitCode;
use std::path::Path;

use crate::qemu_run::{Profile, run_profile};

const KNOWN_V01X_CATEGORIES: &[&str] = &[
    "capability", "cap",  // "cap" is an accepted alias for "capability"
    "ipc", "mmio", "dma", "store", "upgrade",
];

const KNOWN_V02_CATEGORIES: &[&str] = &[
    "lease", "user-copy", "audit", "policy", "evidence", "svc",
];

/// Entry point: `cargo xtask qemu-negative <category>`.
pub fn cmd_qemu_negative(category: Option<&str>) -> ExitCode {
    let category = match category {
        Some(c) => c,
        None => {
            eprintln!("Usage: cargo xtask qemu-negative <category>");
            eprintln!("Known categories (v0.1.x): {}",
                      KNOWN_V01X_CATEGORIES.join(", "));
            eprintln!("Reserved for v0.2:         {}",
                      KNOWN_V02_CATEGORIES.join(", "));
            return ExitCode::FAILURE;
        }
    };

    // If a real profile exists, run it; otherwise emit a placeholder.
    // Both paths share `run_profile` so artefact capture is identical.
    let profile_path = format!("tests/qemu/profiles/{category}.toml");
    if Path::new(&profile_path).exists() {
        // Delegate to the explicit loader via qemu_run::cmd_qemu_run.
        crate::qemu_run::cmd_qemu_run(Some(category))
    } else {
        if !KNOWN_V01X_CATEGORIES.contains(&category)
            && !KNOWN_V02_CATEGORIES.contains(&category)
        {
            eprintln!("[xtask] qemu-negative: unknown category `{category}`");
            return ExitCode::FAILURE;
        }
        println!("[xtask] qemu-negative: no profile for `{category}` \
                  yet — running placeholder (RFC 025 §chicken-and-egg).");
        run_profile(&Profile::negative_placeholder(category))
    }
}
