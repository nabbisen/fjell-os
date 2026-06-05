# Fjell OS

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Build](https://github.com/nabbisen/fjell-os/actions/workflows/ci.yml/badge.svg)](https://github.com/nabbisen/fjell-os/actions/workflows/ci.yml)

**A memory-safe, verifiable, minimal microkernel — built in Rust for industrial and edge systems.**

---

## Overview

Fjell OS is a research operating system that combines:

- **Memory safety by design** — written in Rust 2024 edition, `no_std` kernel
- **Capability-based security** — no ambient `root`; all authority is explicit
- **Minimal microkernel** — drivers, filesystems, and audit services live in user space
- **ABDD** (Accessible by Default and by Design) — services emit structured *intent* streams
  rather than pixel data, letting a Presentation Proxy render them for any modality

Current version: **v0.3.0-alpha.1** — first alpha of the v0.3 line.  
`fjell-trust-provider` and `fjell-keyring` crates land; revised RFC pack
covering v0.3 through v0.9 published.  
v0.2.x history: `TEST:V02:PASS` earned at v0.2.14; v0.2.9–v0.2.23 hardening
line (12 RFCs, 48–059) addressed all 20 post-review findings from v0.2.8.  
Previous milestone: **v0.1.0** (M1–M8 complete, May 2026) — 29/29 QEMU negative-test markers across 9 categories confirmed.  
For full release history see [docs/src/releases/v0.2.0-release-gate.md](docs/src/releases/v0.2.0-release-gate.md).

> ⚠ **v0.1.0 is a local verified prototype, not a production OS.**
> See [docs/src/releases/v0.1.0-limitations.md](docs/src/releases/v0.1.0-limitations.md)
> for what it deliberately is *not* (no production secure boot, no
> remote attestation, no networking, no POSIX). The v0.1.x line
> stabilises this prototype before v0.2 closes the security
> boundary.

---

## Why Fjell OS?

Modern general-purpose operating systems carry decades of implicit trust, memory-unsafe
drivers, and monolithic privilege models that are hard to audit or formally verify.
Fjell OS starts from the smallest defensible core and builds up deliberately:

- Industrial / edge devices need long-term stability and auditability
- Accessible-by-default requires separating *what* from *how it looks*
- Rust's ownership model makes memory invariants enforceable at compile time

See [docs/src/internals/design-philosophy.md](docs/src/internals/design-philosophy.md)
for the full design rationale.

---

## Quick Start

### Prerequisites

```sh
# Rust 1.91 + nightly bootstrap flag (for -Z build-std)
rustup toolchain install 1.91
# Note: rustup target add is NOT needed — build-std compiles core from source

# LLVM linker (ld.lld) — cross-platform, handles RISC-V out of the box
sudo apt-get install lld                       # Ubuntu/Debian  → provides ld.lld
sudo pacman -S lld                             # Arch Linux
brew install llvm && brew link llvm            # macOS

# objcopy (to extract flat binaries from ELFs, needed by cargo xtask build-services)
sudo apt-get install llvm                      # Ubuntu/Debian  → provides llvm-objcopy
sudo pacman -S llvm                            # Arch Linux
# macOS: included with the llvm install above

# Alternative: GNU toolchain (both linker and objcopy in one package)
# sudo apt-get install gcc-riscv64-unknown-elf
# sudo pacman -S riscv64-elf-binutils
# (update .cargo/config.toml linker = "riscv64-unknown-elf-ld" if using this)

# QEMU
sudo apt-get install qemu-system-misc          # Ubuntu/Debian
sudo pacman -S qemu-system-riscv               # Arch Linux
brew install qemu                              # macOS
```

### Build and run

```sh
# 1. Build user-space service binaries (produces crates/fjell-kernel/prebuilt/*.bin)
cargo xtask build-services

# 2. Build fjell-kernel (embeds the prebuilt service binaries)
RUSTC_BOOTSTRAP=1 cargo build \
  --package fjell-kernel \
  --target riscv64gc-unknown-none-elf \
  --release \
  -Z build-std=core,compiler_builtins

# Or: do both in one command and launch QEMU
cargo xtask qemu                  # interactive  — exit with Ctrl-A then X
cargo xtask qemu-test             # smoke test   — checks for TEST:M8:PASS
cargo xtask qemu-test m8          # same as above, explicit milestone
```

> **Note on build order**: `fjell-kernel` embeds pre-built service binaries
> via `include_bytes!("../../prebuilt/*.bin")`.  You must run
> `cargo xtask build-services` (or equivalent) at least once before building
> the kernel.  The kernel's `build.rs` will print a helpful error with
> instructions if the prebuilt binaries are missing.

---

## Design Notes

| Principle | Decision |
|-----------|----------|
| Kernel privilege | S-mode (M-mode shim only) |
| Virtual memory | Sv39, 4 KiB pages, shared kernel map |
| Allocator | Bump (boot) + bitmap frame allocator |
| Scheduler | Fixed-priority round-robin, single hart |
| IPC | Synchronous rendezvous (L4/seL4 style, M3+) |
| Capability | Generation-tagged slot table (M3+) |
| Audit | Append-only fixed-capacity ring |
| Config | Declarative TOML |
| UI boundary | Intent Stream → Presentation Proxy |

For full documentation see [docs/src/SUMMARY.md](docs/src/SUMMARY.md).

---

*Fjell — Norwegian for "mountain".*
