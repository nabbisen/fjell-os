# ADR-0001 — Minimal Microkernel

**Status:** Accepted  
**Date:** 2026-05-04 (v0.1.0) / updated 2026-05-17 (v0.1.4, RFC 045)  
**Supersedes:** ADR-0001 Target Architecture, ADR-0002 Microkernel Boundary

---

## Context

Fjell OS needs an initial hardware target and a principled boundary between
the privileged kernel and user-space code. Both decisions (target
architecture and kernel scope) are tightly coupled — the kernel size target
is dictated by the minimalism principle.

---

## Decision

The kernel implements **only**: address-space isolation, task management,
synchronous IPC, capability enforcement, interrupt routing, and timer
interrupt.

The primary target for v0.1.0 is **`riscv64gc-unknown-none-elf`** running
on **QEMU `virt` machine**.

The kernel does **not** implement: device drivers, filesystems, network
stacks, TOML parsers, audit log formatting, GUI, or service dependency
resolution.

Kernel binary size target: ≤ 64 KiB of `.text` at v0.1.0.

---

## Consequences

- RISC-V is chosen over x86-64 because its clean M/S/U privilege
  architecture has no legacy cruft and is well-documented.
- IPC performance is critical; synchronous rendezvous (L4 style) is used
  to minimise kernel complexity while keeping hot-path latency low.
- `fjell-cap` and `fjell-ipc` are host-testable pure-logic crates with no
  arch dependencies, catching most capability and IPC bugs before QEMU.
- Moving drivers and services to user space means their crashes cannot panic
  the kernel, and their capabilities can be revoked without a reboot.

---

## Security Boundary Impact

The boundary between kernel and user space is the most security-critical
boundary in the system. Keeping the kernel minimal limits the attack
surface that must eventually be formally verified (v0.6 milestone).

Every decision that would move code *into* the kernel must be justified in
a new ADR or RFC.

---

## Deferred Work

- Formal verification of the kernel binary (v0.6).
- Physical hardware target (v0.3 — first ARM or RISC-V board).
- SMP / multi-hart support (no current milestone).
- Demand paging / copy-on-write (no current milestone).

---

## Related RFCs

- RFC 004 (Capability Gating), RFC 014 (Complete Capability Gating)
- RFC 022 (Task Start Entry Validation)
- RFC 031 (Unified Capability Enforcement, v0.2)
