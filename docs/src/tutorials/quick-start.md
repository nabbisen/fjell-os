# Quick Start

**Goal:** reach `TEST:M8:PASS` on a QEMU RISC-V node in five minutes.

## Prerequisites

Ubuntu 24.04 (or compatible), x86_64 host.

```bash
# Rust 1.91 (the pinned Fjell build toolchain) + sources + linker
sudo apt install rustc-1.91 cargo-1.91 rust-1.91-src lld llvm
# QEMU with the riscv64 system emulator (package: qemu-system-misc)
sudo apt install qemu-system-misc
qemu-system-riscv64 --version   # expect 8.2.x
```

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
like this (verified at v0.18.2):

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
