//! QEMU smoke test runner for `cargo xtask qemu-test [milestone]`.

use std::process::{Command, ExitCode};

const TIMEOUT_SECS: &str = "30";

pub fn cmd_qemu_test(milestone: Option<&str>) -> ExitCode {
    let marker = match milestone {
        Some("m2") => "TEST:M2:PASS",
        Some("m3") => "TEST:M3:PASS",
        Some("m4") => "TEST:M4:PASS",
        _          => "TEST:M4:PASS",   // default = current milestone
    };

    let kernel = crate::qemu::build_all();
    println!("[xtask] smoke test — waiting for `{marker}` (timeout {TIMEOUT_SECS}s)");

    let output = Command::new("timeout")
        .args([TIMEOUT_SECS,
               "qemu-system-riscv64",
               "-machine", "virt",
               "-bios",    "none",
               "-nographic",
               "-kernel",  &kernel])
        .output()
        .expect("failed to run qemu-system-riscv64");

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    if combined.contains(marker) {
        println!("[xtask] FOUND `{marker}` — smoke test PASSED ✓");
        ExitCode::SUCCESS
    } else {
        eprintln!("[xtask] `{marker}` NOT found — smoke test FAILED");
        eprintln!("--- QEMU output ---");
        eprintln!("{combined}");
        ExitCode::FAILURE
    }
}
