# Changelog

All notable changes to Fjell OS are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## [Unreleased]

### Added — M1: Bootable Kernel

- `crates/fjell-kernel/link.ld` — linker script placing the image at
  `0x8000_0000` with 4 MiB kernel stack
- `crates/fjell-kernel/src/boot.rs` — `_start` assembly: hart-0 selection,
  BSS zero-fill, stack pointer, jump to `kmain`
- `crates/fjell-kernel/src/uart.rs` — NS16550A UART driver (MMIO `0x1000_0000`),
  `fmt::Write` impl with automatic CRLF on `\n`
- `crates/fjell-kernel/src/console.rs` — `print!` / `println!` macros backed
  by `static mut UART` (single-hart M1 bootstrap; replaced with a lock in M2)
- `crates/fjell-kernel/src/main.rs` — `kmain()` prints boot banner; panic
  handler writes to UART then spins
- `crates/fjell-kernel/.cargo/config.toml` — kernel-local RISC-V target and
  QEMU runner (moved out of workspace root to avoid polluting host builds)
- `crates/fjell-tools/src/main.rs` — `cargo xtask qemu` and
  `cargo xtask qemu-test` subcommands
- `.github/workflows/ci.yml` — CI: host check/test + kernel check/build +
  QEMU M1 smoke test
- Full `docs/` mdBook: design-philosophy, architecture-overview, memory-model,
  task-model, trap-syscall, unsafe-policy, local-development, qemu-tests,
  all reference pages, ADRs 0001–0005

### Fixed — Incorrect code-model Name

- `.cargo/config.toml`: Changed `code-model=medany` → `code-model=medium`.
  Rust/LLVM's `-C code-model=` follows LLVM's naming convention.
  The LLVM/Rust equivalent of GCC's `medany` is `medium`.

- Added new `crates/fjell-kernel/build.rs`.
  Passes `link.ld` to the linker using an absolute path derived from `CARGO_MANIFEST_DIR`
  (`cargo:rustc-link-arg=-T<abs_path>/link.ld`).
  This ensures that linker script symbols such as `__bss_end` and `__stack_top` are reliably resolved
  even when building with `--package fjell-kernel` from the workspace root.

- Added `[target.riscv64gc-unknown-none-elf]` section to `.cargo/config.toml` (workspace root)
  with `code-model=medium` configured.
  This is necessary because rustflags written in a subdirectory's `.cargo/config.toml`
  are not read when building from the workspace root.

- Removed `rustflags` from `crates/fjell-kernel/.cargo/config.toml`
  (it was redundant and ineffective).

- Updated `README.md`, `docs/src/getting-started/quick-start.md`,
  and `docs/src/internals/local-development.md`:
  Explicitly documented that running `cargo build --target riscv64gc-unknown-none-elf`
  (without `--package`) attempts to build all `default-members` for RISC-V,
  causing `std` crates to fail. Added a warning to always specify
  `--package fjell-kernel --target riscv64gc-unknown-none-elf` together for kernel builds.

- Added a comment to `crates/fjell-kernel/.cargo/config.toml` clarifying that
  this file is only effective when running `cargo` within `crates/fjell-kernel/`
  and is not applied when building from the workspace root.

- `Cargo.toml` (workspace): added `default-members` excluding `fjell-kernel`
  so `cargo build` / `cargo check` without flags targets only host-side crates.
  `fjell-kernel` must be built explicitly with
  `--package fjell-kernel --target riscv64gc-unknown-none-elf`.
- `crates/fjell-kernel/src/boot.rs`: wrapped `global_asm!` in
  `#[cfg(target_arch = "riscv64")]` to silence invalid-mnemonic errors when
  the crate is compiled for the host (e.g. via `--workspace`).
- `Cargo.toml` (workspace): edition `"2024"` restored (was incorrectly
  changed to `"2021"` in a previous fix).
- `crates/fjell-kernel/Cargo.toml`: removed `[profile.*]` sections — profiles
  are only honoured at the workspace root.
- `crates/fjell-kernel/src/main.rs`: `#[no_mangle]` →
  `#[unsafe(no_mangle)]` (required by edition 2024).
- `crates/fjell-kernel/src/console.rs`: `static mut UART` →
  `static UART: SyncUnsafeCell` to avoid edition-2024 `static_mut_refs` deny.

---

### Added — M0: Repository Foundation

- Cargo workspace with `resolver = "2"`, Rust 2024 edition, 17 crate skeletons
- `LICENSE` (Apache-2.0), `NOTICE`, `TERMS_OF_USE.md`
- `README.md`, `ROADMAP.md`, `CHANGELOG.md`
- All `no_std` kernel crate stubs with initial type definitions

---

*Releases are tagged once each milestone passes its acceptance criteria.*
