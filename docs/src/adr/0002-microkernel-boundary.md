# ADR-0002 — Microkernel Boundary

**Status:** Superseded — see ADR-0001 Minimal Microkernel (RFC 045)  
**Date:** 2026-05-04

## Context

Where exactly does the kernel end and user space begin?

## Decision

The kernel implements **only**: address-space isolation, task management,
IPC, capability enforcement, interrupt routing, and timer.

The kernel does **not** implement: device drivers, filesystems, network
stacks, TOML parsers, audit log formatting, GUI, or service dependency
resolution.

## Rationale

Every line of code inside the kernel expands the Trusted Computing Base and
the surface that must eventually be formally verified.  The seL4 experience
shows that a kernel of ≈ 10 000 lines of C can be fully verified; a monolithic
kernel cannot.  Rust's type system makes a small kernel even more defensible.

Moving drivers and services to user space means their crashes cannot panic
the kernel, and their capabilities can be revoked without a reboot.

## Consequences

- IPC performance is critical; synchronous rendezvous (L4 style) is chosen
  to minimise kernel complexity while keeping hot-path latency low (ADR-0010).
- The kernel binary size target is ≤ 64 KiB of `.text` for v0.1.0.
- `fjell-cap` and `fjell-ipc` are host-testable pure-logic crates with no
  arch dependencies, so the majority of capability and IPC bugs are caught
  before touching QEMU.
