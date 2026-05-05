//! Fjell OS host-side development tools (`cargo xtask` entry point).
//!
//! Usage: `cargo xtask <subcommand> [args]`
//!
//! Subcommands:
//!   qemu                — build fjell-kernel and launch QEMU interactively
//!   qemu-test [m2|m3]   — run QEMU smoke test, check for TEST:M*:PASS marker

use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("qemu") => cmd_qemu(),
        Some("qemu-test") => cmd_qemu_test(args.get(1).map(String::as_str)),
        Some(other) => {
            eprintln!("fjell-tools: unknown subcommand `{other}`");
            eprintln!("Usage: cargo xtask {{ qemu | qemu-test [milestone] }}");
            ExitCode::FAILURE
        }
        None => {
            eprintln!("fjell-tools: no subcommand given");
            eprintln!("Usage: cargo xtask {{ qemu | qemu-test [milestone] }}");
            ExitCode::FAILURE
        }
    }
}

// ── Subcommand implementations ────────────────────────────────────────────────

/// Build the kernel and launch QEMU interactively.
fn cmd_qemu() -> ExitCode {
    let kernel_elf = build_kernel();

    println!("[xtask] launching QEMU  (Ctrl-A X to exit)");
    let status = Command::new("qemu-system-riscv64")
        .args([
            "-machine", "virt",
            "-bios",    "none",
            "-nographic",
            "-kernel",  &kernel_elf,
        ])
        .status()
        .expect("failed to launch qemu-system-riscv64 — is it installed?");

    if status.success() {
        ExitCode::SUCCESS
    } else {
        eprintln!("[xtask] QEMU exited with status: {status}");
        ExitCode::FAILURE
    }
}

/// Build the kernel, run QEMU with a timeout, and check UART output for
/// the `TEST:M*:PASS` marker.
fn cmd_qemu_test(milestone: Option<&str>) -> ExitCode {
    let marker = match milestone {
        Some("m2") => "TEST:M2:PASS",
        Some("m3") => "TEST:M3:PASS",
        _ => "Fjell OS kernel started",   // M1 smoke test
    };

    let kernel_elf = build_kernel();
    println!("[xtask] smoke test — waiting for `{marker}`");

    // Use `timeout` to kill QEMU after 10 s if the marker never appears.
    let output = Command::new("timeout")
        .args([
            "10",
            "qemu-system-riscv64",
            "-machine", "virt",
            "-bios",    "none",
            "-nographic",
            "-kernel",  &kernel_elf,
        ])
        .output()
        .expect("failed to run qemu-system-riscv64");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    if combined.contains(marker) {
        println!("[xtask] FOUND: `{marker}` — test PASSED");
        ExitCode::SUCCESS
    } else {
        eprintln!("[xtask] marker `{marker}` NOT found — test FAILED");
        eprintln!("--- QEMU output ---");
        eprintln!("{combined}");
        ExitCode::FAILURE
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build `fjell-kernel` for `riscv64gc-unknown-none-elf` and return the path
/// to the resulting ELF binary.
fn build_kernel() -> String {
    println!("[xtask] building fjell-kernel …");
    let status = Command::new("cargo")
        .args([
            "build",
            "--package", "fjell-kernel",
            "--target",  "riscv64gc-unknown-none-elf",
            "--release",
        ])
        .status()
        .expect("cargo build failed");

    if !status.success() {
        eprintln!("[xtask] cargo build failed");
        std::process::exit(1);
    }

    "target/riscv64gc-unknown-none-elf/release/fjell-kernel".to_string()
}
