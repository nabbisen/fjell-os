//! Interactive QEMU launcher for `cargo xtask qemu`.

use std::process::{Command, ExitCode};

/// Build the kernel and launch QEMU interactively.
pub fn cmd_qemu() -> ExitCode {
    let kernel_elf = build_kernel();

    println!("[xtask] launching QEMU");
    println!("[xtask] To exit: press Ctrl-A, release, then press X");
    println!();

    let status = Command::new("qemu-system-riscv64")
        .args([
            "-machine", "virt",
            "-bios", "none",
            "-nographic",
            "-kernel", &kernel_elf,
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

/// Build `fjell-kernel` for `riscv64gc-unknown-none-elf` in release mode.
/// Returns the path to the resulting ELF binary.
pub fn build_kernel() -> String {
    println!("[xtask] building fjell-kernel ...");
    let status = Command::new("cargo")
        .args([
            "build",
            "--package", "fjell-kernel",
            "--target", "riscv64gc-unknown-none-elf",
            "--release",
        ])
        .status()
        .expect("cargo build failed to start");

    if !status.success() {
        eprintln!("[xtask] cargo build failed");
        std::process::exit(1);
    }

    "target/riscv64gc-unknown-none-elf/release/fjell-kernel".to_string()
}
