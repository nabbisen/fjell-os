# Quick Start

**Goal:** reach `TEST:M8:PASS` on a QEMU RISC-V node in five minutes.

## Prerequisites

Ubuntu 24.04 (or compatible), x86_64 host.

```bash
# Rust, through rustup. Do not install Rust from apt: Ubuntu's `rust-src`
# package ships the standard library's source without its `Cargo.lock`, and
# this kernel is built with `-Z build-std`, which needs both. An apt
# toolchain fails with
#   error: ".../library/Cargo.lock" does not exist, unable to build with
#          the standard library
# which is the error that kept this project's CI red for four months
# (ERRATA E-041). rustup is the only path that works.
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup toolchain install 1.98.1 \
    --component rust-src \
    --target riscv64gc-unknown-none-elf

# The linker and binary tools the kernel build needs. `ld.lld` is in the
# `lld` package, not `llvm`; `llvm` provides llvm-objcopy and llvm-nm.
# These are not Rust, so apt is the right source for them.
sudo apt install lld llvm
# QEMU with the riscv64 system emulator (package: qemu-system-misc)
sudo apt install qemu-system-misc
qemu-system-riscv64 --version   # expect 8.2.x
```

Once you have cloned the repository, `rust-toolchain.toml` is what decides
the toolchain: rustup reads it and installs the exact version, components
and target the project declares, so the version above is a starting point
rather than a second declaration.

## Build

```bash
git clone https://github.com/nabbisen/fjell-os
cd fjell-os
cargo xtask build
```

## Run the smoke test

```bash
cargo xtask qemu-test m8
```

The build compiles all service binaries for `riscv64gc-unknown-none-elf`,
embeds them into the kernel, and boots QEMU `virt`. Early boot output looks
like this (example from v0.20.0 era):

```text
Fjell OS kernel started.
mode: S
platform: qemu-virt
memory: detected (128 MiB)
mm: boot allocator ready
mm: frame allocator ready  (32159 free frames)
vm: sv39 enabled
trap: stvec installed
M3: capability table initialized
M3: endpoint table initialized
...
TEST:M8:PASS
```

The xtask exits successfully when the `TEST:M8:PASS` marker is matched:

```text
[xtask] profile `smoke-m8` PASS (1 marker(s) matched) ✓
```

## Where to go next

- Run the full local gate: `cargo xtask test-all` (host tests, property
  tests, audits, reproducibility, all QEMU smoke and negative tiers).
- Write your first service: [Writing a Service](../sdk/writing-a-service.md).
- Understand what just booted: [Architecture Overview](../architecture/overview.md).
