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
    "fjell-neg-test",
    "fjell-svc-timeout",
    "fjell-svc-fault",
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
    // v0.7 Distributed Sync
    "fjell-syncd",
    // v0.4 Networking
    "fjell-driver-virtio-net",
    "fjell-netd",
    "fjell-secure-transportd",
    "fjell-diagnosticsd",
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

    // Determine the BSS end address from the ELF so we can pad the flat binary
    // to cover BSS pages.  If .bss falls in a page beyond the last PROGBITS
    // section, objcopy -O binary (which excludes NOBITS) would produce a binary
    // too small to trigger a second-page mapping in spawn.rs.  We pad with
    // zeros so spawn maps the right number of pages and BSS is zero-filled.
    let pad_to: Option<String> = bss_end_page_aligned(elf);
    if let Some(ref addr) = pad_to {
        println!("[xtask]     BSS end → padding flat binary to {addr}");
    }

    for prog in &candidates {
        let mut cmd = Command::new(prog);
        cmd.args(["-O", "binary"]);
        if let Some(ref addr) = pad_to {
            // --pad-to extends the binary with the gap-fill byte (default 0)
            // up to the given address, covering any BSS pages.
            cmd.arg("--gap-fill").arg("0");
            cmd.arg(format!("--pad-to={addr}"));
        }
        cmd.arg(elf).arg(flat);
        match cmd.status() {
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

/// Read ELF symbols `__bss_start` / `__bss_end` and PROGBITS section boundaries
/// to determine whether service BSS falls in a page beyond the flat binary.
/// If so, returns the pad-to address (end of the last BSS page) as a hex string
/// for use with `objcopy --pad-to`.
///
/// Returns None if BSS fits within the binary's existing pages (no pad needed).
fn bss_end_page_aligned(elf: &PathBuf) -> Option<String> {
    const PAGE: u64 = 0x1000;

    // --- 1. Get __bss_start and __bss_end from nm --------------------------
    let nm_candidates = ["riscv64-linux-gnu-nm", "riscv64-unknown-elf-nm",
                         "llvm-nm", "nm"];
    let nm_out = nm_candidates.iter().find_map(|t| {
        Command::new(t).arg(elf).output().ok().filter(|o| o.status.success())
    })?;
    let nm_str = String::from_utf8_lossy(&nm_out.stdout);

    let get_sym = |name: &str| -> Option<u64> {
        nm_str.lines()
            .find(|l| l.split_whitespace().last() == Some(name))
            .and_then(|l| l.split_whitespace().next())
            .and_then(|h| u64::from_str_radix(h, 16).ok())
    };

    let bss_start = get_sym("__bss_start")?;
    let bss_end   = get_sym("__bss_end")?;

    // If BSS is empty and both symbols are the same, treat as no BSS.
    // Code accessing the address at bss_start still needs the page mapped
    // because the runtime may compute a pointer to __bss_start.
    // So we proceed even for empty BSS if it starts at a new page.

    // --- 2. Get highest ALLOC PROGBITS section end -------------------------
    let re_candidates = ["riscv64-linux-gnu-readelf", "riscv64-unknown-elf-readelf",
                         "llvm-readelf"];
    let re_out = re_candidates.iter().find_map(|t| {
        Command::new(t).args(["-S", "--wide"]).arg(elf)
            .output().ok().filter(|o| o.status.success())
    })?;
    let re_str = String::from_utf8_lossy(&re_out.stdout);

    // readelf -S --wide line format (space-split tokens):
    //  [Nr] Name Type Address Offset Size ES Flg ...
    //   0    1    2    3      4      5    6  7
    // After splitting by whitespace:
    //  "[" "Nr]" Name Type Address Offset Size ES Flg...
    //   0    1    2    3    4       5      6    7  8
    let progbits_end: u64 = re_str.lines()
        .filter(|l| l.contains("PROGBITS") && l.contains(" A"))  // ALLOC flag
        .filter_map(|l| {
            let p: Vec<&str> = l.split_whitespace().collect();
            // Address at index 4, Size at index 6
            let addr = u64::from_str_radix(p.get(4)?, 16).ok()?;
            let size = u64::from_str_radix(p.get(6)?, 16).ok()?;
            // Skip sections at address 0 (debug/comment sections)
            if addr == 0 { return None; }
            Some(addr + size)
        })
        .max()
        .unwrap_or(0);

    if progbits_end == 0 { return None; }

    // --- 3. Decide whether to pad ------------------------------------------
    // Round progbits_end up to the page boundary (= exclusive end of the
    // last binary page loaded by spawn).
    let binary_page_end = (progbits_end + PAGE - 1) & !(PAGE - 1);

    // KEY: compare bss_END (not bss_start) against binary_page_end.
    // A service may have bss_start inside the binary's pages (e.g. at 0x40248)
    // but bss_end far beyond (e.g. at 0x4a000) when static buffers are large.
    // Using bss_start for the check caused false-None returns in that case.
    let bss_cover_end = if bss_end <= bss_start {
        // Empty BSS — if it sits exactly at a page boundary we still want
        // that page mapped so runtime code using __bss_start doesn't fault.
        let bss_page_start = bss_start & !(PAGE - 1);
        bss_page_start + PAGE
    } else {
        // Non-empty BSS — round bss_end up to the next page boundary.
        // This gives the exclusive end of the last BSS page.
        (bss_end + PAGE - 1) & !(PAGE - 1)
    };

    if bss_cover_end <= binary_page_end {
        // BSS is entirely within the pages the binary already provides.
        eprintln!("[bss-pad] {}: bss=[0x{:x}..0x{:x}] cover=0x{:x} <= binary_page_end=0x{:x} → no pad",
            elf.file_name().unwrap_or_default().to_string_lossy(),
            bss_start, bss_end, bss_cover_end, binary_page_end);
        return None;
    }

    eprintln!("[bss-pad] {}: bss=[0x{:x}..0x{:x}] binary_end=0x{:x} → pad to 0x{:x} ({} pages)",
        elf.file_name().unwrap_or_default().to_string_lossy(),
        bss_start, bss_end, binary_page_end, bss_cover_end,
        (bss_cover_end - 0x40000 + PAGE - 1) / PAGE);

    Some(format!("0x{bss_cover_end:x}"))
}
