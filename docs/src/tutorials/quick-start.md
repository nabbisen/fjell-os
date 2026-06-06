# Quick Start

**Goal:** reach `TEST:M8:PASS` on a QEMU RISC-V node in five minutes.

## Prerequisites

```bash
# Install Rust 1.91
sudo apt install rustc-1.91   # or use rustup
# Install QEMU
sudo apt install qemu-system-riscv64
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
# Expected: TEST:M8:PASS
```

*TODO: Verify output lines. References RFC 025.*
