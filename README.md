# Fjell OS

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Build](https://github.com/nabbisen/fjell-os/actions/workflows/ci.yml/badge.svg)](https://github.com/nabbisen/fjell-os/actions/workflows/ci.yml)

**A memory-safe, verifiable, minimal microkernel — built in Rust for
industrial and edge systems.**

---

## Overview

Fjell OS is a research operating system that combines:

- **Memory safety by design** — written in Rust 2024 edition, `no_std` kernel
- **Capability-based security** — no ambient `root`; all authority is explicit
- **Minimal microkernel** — drivers, filesystems, and audit services live in user space
- **ABDD** (Accessible by Default and by Design) — services emit structured *intent* streams rather than pixel data, letting a Presentation Proxy render them for any modality

The current milestone is **v0.1.0**, targeting RISC-V 64 on QEMU `virt`.

---

## Why Fjell OS?

Modern general-purpose operating systems carry decades of implicit trust,
memory-unsafe drivers, and monolithic privilege models that are hard to audit
or formally verify.  Fjell OS starts from the smallest defensible core and
builds up deliberately:

- Industrial / edge devices need long-term stability and auditability
- Accessible-by-default requires separating *what* from *how it looks*
- Rust's ownership model makes memory invariants enforceable at compile time

See [docs/src/internals/design-philosophy.md](docs/src/internals/design-philosophy.md)
for the full design rationale.

---

## Quick Start

### Prerequisites

```sh
# Rust 1.91 + bare-metal RISC-V target
rustup toolchain install 1.91
rustup target add riscv64gc-unknown-none-elf

# QEMU (Ubuntu/Debian)
sudo apt-get install qemu-system-misc
# QEMU (Arch Linux)
sudo pacman -S qemu-system-riscv
# QEMU (macOS)
brew install qemu
# RISC-V GCC linker — required for kernel link step (Ubuntu/Debian)
sudo apt-get install gcc-riscv64-unknown-elf
# (Arch Linux)
sudo pacman -S riscv64-elf-gcc
```

### Build and Run

```sh
# ── Host-side crates (services and utilities) ─────────────────────────────
cargo check
cargo build

# ── Kernel (RISC-V cross-compilation) ─────────────────────────────────────
# --package and --target must always be specified together.
# Specifying only --target or only --package will affect other crates and cause failures.
cargo check --package fjell-kernel --target riscv64gc-unknown-none-elf
cargo build --package fjell-kernel --target riscv64gc-unknown-none-elf --release

# ── Launch with QEMU (executes the build above internally, then starts QEMU) ───
cargo xtask qemu        # Interactive launch (exit with Ctrl-A X)
cargo xtask qemu-test   # Smoke test (with timeout, non-interactive)
```

> **Common Mistake**
> 
> Running `cargo build --target riscv64gc-unknown-none-elf` (without `--package`) attempts to build all `default-members` for RISC-V, causing errors in service crates that depend on `std`. Always include `--package fjell-kernel` when building the kernel.

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

For more detail, see our [full documentation](docs/src/SUMMARY.md).

---

*Fjell — Norwegian for "mountain".*
