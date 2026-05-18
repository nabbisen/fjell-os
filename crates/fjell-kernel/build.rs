//! Build script for fjell-kernel.
//!
//! 1. Passes the linker script with an absolute path (works from any CWD).
//! 2. Checks that `prebuilt/` contains the flat service binaries and prints
//!    a helpful error if they are missing.
//!
//! To populate `prebuilt/` before building the kernel:
//!   cargo xtask build-services
//!
//! Or manually:
//!   RUSTC_BOOTSTRAP=1 cargo build --release \
//!     --target riscv64gc-unknown-none-elf \
//!     -Z build-std=core,compiler_builtins \
//!     -p fjell-init -p fjell-configd -p fjell-cap-broker \
//!     -p fjell-auditd -p fjell-service-manager -p fjell-sample-service
//!   for s in fjell-init fjell-configd fjell-cap-broker \
//!             fjell-auditd fjell-service-manager fjell-sample-service; do
//!     riscv64-unknown-elf-objcopy -O binary \
//!       target/riscv64gc-unknown-none-elf/release/$s \
//!       crates/fjell-kernel/prebuilt/$s.bin
//!   done

use std::env;
use std::path::PathBuf;

const SERVICES: &[&str] = &[
    "fjell-init",
    "fjell-configd",
    "fjell-cap-broker",
    "fjell-auditd",
    "fjell-service-manager",
    "fjell-sample-service",
];

fn main() {
    let manifest = env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR not set");
    let manifest = PathBuf::from(&manifest);

    // ── 1. Pass the linker script ─────────────────────────────────────────────
    // Absolute path so the flag works regardless of the working directory.
    println!("cargo:rustc-link-arg=-T{}/link.ld", manifest.display());
    println!("cargo:rerun-if-changed=link.ld");

    // ── 2. Validate prebuilt binaries exist ───────────────────────────────────
    let prebuilt = manifest.join("prebuilt");
    println!("cargo:rerun-if-changed={}", prebuilt.display());

    let mut missing = Vec::new();
    for svc in SERVICES {
        let bin = prebuilt.join(format!("{svc}.bin"));
        if !bin.exists() {
            missing.push(*svc);
        }
        println!("cargo:rerun-if-changed={}", bin.display());
    }

    if !missing.is_empty() {
        println!("cargo:warning=\n\
            ┌─────────────────────────────────────────────────────────────┐\n\
            │  fjell-kernel: prebuilt service binaries not found          │\n\
            │                                                              │\n\
            │  Run the following command to build them:                    │\n\
            │    cargo xtask build-services                               │\n\
            │                                                              │\n\
            │  Missing: {missing:?}\n\
            └─────────────────────────────────────────────────────────────┘");
        // Emit an include_bytes path that won't exist, producing a compile error
        // with a useful message rather than a silent wrong binary.
        panic!(
            "prebuilt service binaries missing: {missing:?}\n\
             Run `cargo xtask build-services` first."
        );
    }
}
