//! Interactive QEMU launcher and kernel build helpers for `cargo xtask`.

use std::process::{Command, ExitCode};
use std::path::PathBuf;
use std::env;

pub const TARGET:  &str = "riscv64gc-unknown-none-elf";
pub const KERNEL_ELF: &str =
    "target/riscv64gc-unknown-none-elf/release/fjell-kernel";

pub const SERVICES: &[&str] = &[
    "fjell-init",
    "fjell-configd",
    "fjell-cap-broker",
    "fjell-auditd",
    "fjell-service-manager",
    "fjell-sample-service",
    // M5
    "fjell-semantic-stream",
    "fjell-proxy-text",
    // M6
    "fjell-devmgr",
    "fjell-driver-virtio-blk",
    "fjell-storaged",
    "fjell-bootctl",
    "fjell-upgraded",
    "fjell-powerd",
    // M7
    "fjell-verifyd",
    "fjell-rootfsd",
    "fjell-snapshotd",
    // M8
    "fjell-measuredd",
    "fjell-attestd",
    "fjell-recoveryd",
];

/// Build user-space service binaries, extract flat images to `prebuilt/`.
///
/// Must be run before `build_kernel` when starting from a clean checkout.
pub fn build_services() -> bool {
    println!("[xtask] building service crates ({TARGET} release) ...");

    // Build all service ELFs.
    let status = cargo_cmd()
        .args(["build", "--target", TARGET, "--release",
               "-Z", "build-std=core,compiler_builtins"])
        .args(SERVICES.iter().flat_map(|s| ["--package", s]))
        .env("RUSTC_BOOTSTRAP", "1")
        .status()
        .expect("cargo build failed to start");

    if !status.success() {
        eprintln!("[xtask] service build FAILED");
        return false;
    }

    // Extract flat binaries.
    let bin_dir = PathBuf::from("target").join(TARGET).join("release");
    let prebuilt = PathBuf::from("crates/fjell-kernel/prebuilt");
    std::fs::create_dir_all(&prebuilt).ok();

    for svc in SERVICES {
        let elf  = bin_dir.join(svc);
        let flat = prebuilt.join(format!("{svc}.bin"));
        println!("[xtask]   objcopy {svc} → prebuilt/{svc}.bin");
        if !run_objcopy(&elf, &flat) {
            return false;
        }
    }
    println!("[xtask] service binaries ready in crates/fjell-kernel/prebuilt/");
    true
}

/// Build `fjell-kernel` in release mode.
/// Returns the path to the kernel ELF on success.
pub fn build_kernel() -> String {
    println!("[xtask] building fjell-kernel ...");

    // Skip auto-service rebuild in build.rs since xtask handles it.
    let status = cargo_cmd()
        .args(["build",
               "--package", "fjell-kernel",
               "--target", TARGET,
               "--release",
               "-Z", "build-std=core,compiler_builtins"])
        .env("RUSTC_BOOTSTRAP", "1")
        .env("FJELL_SKIP_SERVICE_BUILD", "1")
        .status()
        .expect("cargo build failed to start");

    if !status.success() {
        eprintln!("[xtask] cargo build failed");
        std::process::exit(1);
    }
    KERNEL_ELF.to_string()
}

/// Full build: services first, then kernel.
pub fn build_all() -> String {
    if !build_services() {
        eprintln!("[xtask] aborting: service build failed");
        std::process::exit(1);
    }
    build_kernel()
}

/// Build the kernel and launch QEMU interactively.
pub fn cmd_qemu() -> ExitCode {
    let kernel = build_all();
    println!("[xtask] launching QEMU  (exit: Ctrl-A then X)");
    println!();
    // Create a 16 MiB disk image if it does not exist.
    let disk = "fjell-disk.img";
    if !std::path::Path::new(disk).exists() {
        let _ = Command::new("qemu-img")
            .args(["create", "-f", "raw", disk, "16M"])
            .status();
    }
    let status = Command::new("qemu-system-riscv64")
        .args(["-machine", "virt", "-bios", "none",
               "-nographic", "-kernel", &kernel,
               "-drive", &format!("file={disk},format=raw,if=none,id=hd0"),
               "-device", "virtio-blk-device,drive=hd0"])
        .status()
        .expect("failed to launch qemu-system-riscv64 — is it installed?");

    if status.success() { ExitCode::SUCCESS } else {
        eprintln!("[xtask] QEMU exited with: {status}");
        ExitCode::FAILURE
    }
}

fn cargo_cmd() -> Command {
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    Command::new(cargo)
}

/// Try multiple objcopy tool names (LLVM and GNU variants).
///
/// Priority: $FJELL_OBJCOPY > llvm-objcopy > riscv64-unknown-elf-objcopy >
///           llvm-objcopy-{18,17,16}
fn run_objcopy(elf: &PathBuf, flat: &PathBuf) -> bool {
    let candidates: Vec<String> = if let Ok(v) = std::env::var("FJELL_OBJCOPY") {
        vec![v]
    } else {
        let mut c = vec![
            "llvm-objcopy".to_string(),
            "riscv64-unknown-elf-objcopy".to_string(),
        ];
        for v in [18u32, 17, 16, 15] { c.push(format!("llvm-objcopy-{v}")); }
        c
    };

    for prog in &candidates {
        match Command::new(prog).args(["-O", "binary"]).arg(elf).arg(flat).status() {
            Ok(s) if s.success() => return true,
            Ok(_) => {
                eprintln!("[xtask] {prog} failed (exit non-zero)");
                return false;
            }
            Err(_) => continue, // binary not found; try next
        }
    }

    eprintln!("[xtask] ERROR: no objcopy found.");
    eprintln!("[xtask]   Ubuntu/Debian: sudo apt install llvm");
    eprintln!("[xtask]   Arch:          sudo pacman -S llvm");
    eprintln!("[xtask]   macOS:         brew install llvm");
    eprintln!("[xtask]   Alternative:   sudo apt install gcc-riscv64-unknown-elf");
    eprintln!("[xtask]   Override:      FJELL_OBJCOPY=/path/to/objcopy cargo xtask build-services");
    false
}
