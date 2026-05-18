//! QEMU smoke test runner for `cargo xtask qemu-test [milestone]`.
//!
//! Thin wrapper around `qemu_run::Profile::smoke` + `run_profile`.
//! The milestone → marker mapping is preserved verbatim from the
//! v0.1.0 runner; only the execution path is shared with negative
//! tests (RFC 025).

use std::process::ExitCode;

use crate::qemu::build_all;
use crate::qemu_run::{Profile, run_profile};

pub fn cmd_qemu_test(milestone: Option<&str>) -> ExitCode {
    let (mid, marker) = match milestone {
        Some("m1") => ("m1", "TEST:M1:PASS"),
        Some("m2") => ("m2", "TEST:M2:PASS"),
        Some("m3") => ("m3", "TEST:M3:PASS"),
        Some("m4") => ("m4", "TEST:M4:PASS"),
        Some("m5") => ("m5", "TEST:M5:PASS"),
        Some("m6") => ("m6", "TEST:M6:PASS"),
        Some("m7") => ("m7", "TEST:M7:PASS"),
        Some("m8") => ("m8", "TEST:M8:PASS"),
        _          => ("m8", "TEST:M8:PASS"), // default = current milestone
    };

    // Smoke always rebuilds before running so the test reflects the
    // current source tree.
    let _ = build_all();

    let profile = Profile::smoke(mid, marker);
    run_profile(&profile)
}
