//! QEMU smoke test runner for `cargo xtask qemu-test [milestone]`.

use std::process::{Command, ExitCode};
use std::path::Path;

const TIMEOUT_SECS: &str = "60";

pub fn cmd_qemu_test(milestone: Option<&str>) -> ExitCode {
    let marker = match milestone {
        Some("m2") => "TEST:M2:PASS",
        Some("m3") => "TEST:M3:PASS",
        Some("m4") => "TEST:M4:PASS",
        Some("m5") => "TEST:M5:PASS",
        Some("m6") => "TEST:M6:PASS",
        _          => "TEST:M6:PASS",
    };

    let kernel = crate::qemu::build_all();
    println!("[xtask] smoke test — waiting for `{marker}` (timeout {TIMEOUT_SECS}s)");

    // Create / reset disk image (required for M6 virtio-blk).
    let disk = "fjell-disk.img";
    if Path::new(disk).exists() { let _ = std::fs::remove_file(disk); }
    let _ = Command::new("qemu-img").args(["create", "-f", "raw", disk, "16M"]).status();

    let output = Command::new("timeout")
        .args([TIMEOUT_SECS,
               "qemu-system-riscv64",
               "-machine", "virt",
               "-bios",    "none",
               "-nographic",
               "-kernel",  &kernel,
               "-drive",   &format!("file={disk},format=raw,if=none,id=hd0"),
               "-device",  "virtio-blk-device,drive=hd0"])
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
