//! QEMU smoke test runner for `cargo xtask qemu-test [milestone]`.
//!
//! Builds the kernel, runs it under QEMU with a timeout, and checks that
//! a milestone-specific marker string appears in UART output.

use std::process::{Command, ExitCode};

/// Smoke-test timeout in seconds.
const TIMEOUT_SECS: &str = "30";

/// Run QEMU non-interactively and check for the expected output marker.
pub fn cmd_qemu_test(milestone: Option<&str>) -> ExitCode {
    let marker = match milestone {
        Some("m2") => "TEST:M2:PASS",
        Some("m3") => "TEST:M3:PASS",
        _ => "Fjell OS kernel started",   // M1 / default
    };

    let kernel_elf = crate::qemu::build_kernel();
    println!("[xtask] smoke test — waiting for `{marker}` (timeout {TIMEOUT_SECS}s)");

    let output = Command::new("timeout")
        .args([
            TIMEOUT_SECS,
            "qemu-system-riscv64",
            "-machine", "virt",
            "-bios", "none",
            "-nographic",
            "-kernel", &kernel_elf,
        ])
        .output()
        .expect("failed to run qemu-system-riscv64");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    if combined.contains(marker) {
        println!("[xtask] FOUND `{marker}` — smoke test PASSED");
        ExitCode::SUCCESS
    } else {
        eprintln!("[xtask] `{marker}` NOT found — smoke test FAILED");
        eprintln!("--- QEMU output ---");
        eprintln!("{combined}");
        ExitCode::FAILURE
    }
}
