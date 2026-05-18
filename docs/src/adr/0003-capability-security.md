# ADR-0003 — Capability-Based Security

**Status:** Superseded — see ADR-0002 and ADR-0003 (RFC 045)  
**Date:** 2026-05-04

## Context

How does Fjell OS control which code can access which resource?

## Decision

All resource authority is carried by **unforgeable, kernel-managed
capabilities**.  There is no `root`, no UID/GID, and no ambient authority.
A process can only perform an operation if it holds a capability that grants
the corresponding right.

## Rationale

Ambient authority (POSIX `root`, Windows `SeDebugPrivilege`) means that any
process that achieves privilege escalation obtains *all* authority.
Object-capability systems bound the blast radius of any single compromise to
the authority that was explicitly delegated to the compromised component.

seL4, Capsicum, and CHERI all demonstrate that capability models are
practical.  Rust's ownership model is a compile-time analogue of the same
principle and makes the implementation natural.

## Key properties

- Capabilities are opaque handles; user space cannot forge or inspect the
  kernel's internal representation.
- A child capability's rights are always a subset of its parent's rights
  (`child.rights ⊆ parent.rights`).
- `cap_revoke` removes all descendants; `cap_delete` removes only the
  target slot.
- Generation-tagged handles (`CapHandle` = slot index + generation counter)
  prevent stale-handle reuse after a slot is recycled.

## Consequences

- Implemented in M3.  M2 establishes the task and memory isolation that
  capabilities will govern.
- `fjell-cap` is a pure-logic, host-testable crate so derivation-tree
  invariants can be property-tested with `proptest` before kernel integration.
