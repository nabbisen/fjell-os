# ADR-0002 — Capability-Based IPC

**Status:** Accepted  
**Date:** 2026-05-04 (v0.1.0) / updated 2026-05-17 (v0.1.4, RFC 045)  
**Supersedes:** ADR-0003 Capability-Based Security

---

## Context

How does Fjell OS control which code can access which resource, and how do
services communicate?

---

## Decision

All resource authority is carried by **unforgeable, kernel-managed
capabilities**. There is no `root`, no UID/GID, and no ambient authority.
A process can only perform an operation if it holds a capability that grants
the corresponding right.

IPC is the **only** inter-task communication mechanism. Tasks communicate
by passing messages through kernel-managed endpoints. No shared memory IPC
is provided without a capability.

Capabilities are **transferable only via IPC** (cap_copy / cap_mint over
a trusted endpoint) or via the bootstrap path at task creation.

---

## Consequences

- The kernel's entire authority surface is the capability table; there is
  no ambient privilege path.
- Blast radius of a compromise is bounded by what was explicitly delegated.
- seL4, Capsicum, and CHERI demonstrate the model is practical.
- Rust's ownership model is a compile-time analogue; the implementation
  is natural.
- `fjell-cap`, `fjell-ipc`, `fjell-syscall` are pure-logic, host-testable
  crates.
- All capability checks must eventually go through `require_cap()` (v0.2).
  At v0.1.x, some checks are still type-only.

---

## Security Boundary Impact

This ADR defines the authority model. Every enforcement gap is a security
boundary gap. At v0.1.x:

- Rights bits exist in the ABI type but are not all checked (see RFC 029
  enforcement audit).
- Lease checks exist in the ABI type but are not yet connected to
  every use site (v0.2: RFC 033).

---

## Deferred Work

- `require_cap()` — unified enforcement function (v0.2: RFC 031).
- Lease epoch revocation connected to all use sites (v0.2: RFC 033).
- Default-deny `cap-broker` (v0.2: RFC 040).
- Formal proof of the capability model (v0.6).

---

## Related RFCs

- RFC 004, RFC 006, RFC 010, RFC 014, RFC 015
- RFC 031, RFC 032, RFC 033 (v0.2 enforcement closure)
- RFC 040 (cap-broker default deny, v0.2)
