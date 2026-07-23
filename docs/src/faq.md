# Frequently Asked Questions

## Is Fjell a POSIX OS?

No. Fjell does not implement `read(2)`, `write(2)`, `fork(2)`, or file descriptors. Authority is granted via capability handles, not ambient descriptors. See [Non-Goals](./intro/non-goals.md).

## Can I run Linux software on Fjell?

Not directly. Linux software expects POSIX semantics. Fjell services are authored against `fjell-sdk`. See [Writing a Service](./sdk/writing-a-service.md).

## What architectures are supported?

v0.9: RISC-V RV64GC (QEMU `virt`). v0.12 adds the first real RISC-V board. ARM64 is deferred post-v1.0.

## Where is the kernel source?

`crates/fjell-kernel/` — all code in Rust with `#![forbid(unsafe_code)]` except audited boundaries under `docs/src/verification/unsafe-charter.md`.

## Why RISC-V and not x86-64?

RISC-V has a clean privilege architecture with no legacy baggage. The M/S/U mode split, `satp`/Sv39, and CLINT are straightforward to reason about. See [ADR-0001](./adr/0001-minimal-microkernel.md).

## Does Fjell run on real hardware?

Not in v0.20. The validated profile is QEMU `virt`. VisionFive 2 is provisional. See [v1.0 limitations](../../docs/release/v1-limitations.md).

## Why no kernel heap?

A general `malloc` inside the kernel makes memory ownership harder to reason about and complicates formal verification. All kernel data structures use fixed-capacity tables allocated from the `BootAllocator` during init.

## Why does Fjell use capabilities instead of UNIX permissions?

UNIX permissions tie authority to user identity and process credentials — both are ambient and hard to revoke precisely. Capabilities are object references: you can only act on what you hold, authority can be scoped and revoked with a single `sys_lease_revoke`, and the kernel enforces the boundary without policy decisions.

## Where does the name come from?

*Fjell* is Norwegian for "mountain": solid, minimal, enduring.
