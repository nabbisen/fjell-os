# ADR-0001 — Target Architecture: RISC-V 64 + QEMU

**Status:** Superseded — see ADR-0001 Minimal Microkernel (RFC 045)  
**Date:** 2026-05-04

## Context

Fjell OS needs an initial hardware target for v0.1.0.  The two realistic
options are RISC-V 64 and x86-64, both runnable under QEMU without physical
hardware.

## Decision

The primary target for v0.1.0 is **`riscv64gc-unknown-none-elf`** running on
**QEMU `virt` machine**.

## Rationale

- RISC-V has a clean, well-documented privilege architecture (M/S/U modes,
  `stvec`, `satp`, CLINT, PLIC) with no legacy cruft.
- Sv39 virtual memory is straightforward to implement and reason about.
- QEMU `virt` provides DTB, 16550A UART, and virtio — all we need for v0.1.0.
- RISC-V PMP aligns well with Fjell's long-term hardware-capability goals.
- CI is fully reproducible: `qemu-system-riscv64` is available on all major
  Linux distributions.

## Consequences

- The `riscv64gc-unknown-none-elf` target must be installed via `rustup`.
- Architecture-specific code is isolated in `fjell-arch/src/riscv64/`.
- x86-64 support is deferred to a future milestone (post v0.1.0).
