# ADR-v0.5-003 — Architecture Boundary via Monomorphised Trait

**Status:** Accepted  
**Date:** 2026-05-19 (v0.5.0, RFC v0.5-003)

## Context

All RISC-V-specific code was scattered through `fjell-kernel`, `fjell-arch`, and
various services.  Adding ARM64 would require auditing and forking large sections.

## Decision

Define an `Arch` trait in `fjell-arch` (arch-neutral crate).  All arch-specific code
moves into `fjell-arch-riscv64` and `fjell-arch-arm64` (stub for now).  The kernel
and services import `fjell-arch` only; they never import RISC-V crate names directly.

The trait is monomorphised at compile time via a top-level `type ActiveArch = ...`
alias in `fjell-arch`, so there is no vtable overhead.

## Consequences

- Second-platform preparation is a crate boundary, not a grep-and-edit.
- Unsafe RISC-V intrinsics are confined to `fjell-arch-riscv64`.
- `fjell-arch-arm64` compiles as a stub today; it becomes a build target in v0.6+.
