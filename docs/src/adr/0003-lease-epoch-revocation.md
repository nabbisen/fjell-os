# ADR-0003 — Lease Epoch Revocation

**Status:** Accepted  
**Date:** 2026-05-17 (v0.1.4, RFC 045) — captures decision made in M3/M4

---

## Context

Capabilities must be revocable. Without revocation, a delegated capability
persists even if the delegating service is compromised, exits, or changes
policy. The design question is: how is revocation implemented efficiently
without walking every capability table?

---

## Decision

Revocation is implemented via **lease epoch stamps**.

A `LeaseObject` carries a monotonic `epoch: u32`.
Every lease-bound capability carries a `LeaseBinding { lease_id, epoch_at_issue }`.

Revocation is O(1):

```
lease.epoch = lease.epoch.wrapping_add(1)
lease.state = LeaseState::Revoked
```

Existing capabilities are not deleted; they fail their next use-time check
when `binding.epoch_at_issue != lease.epoch`.

Recursive policy-level revocation belongs to `cap-broker`, not the kernel.
The kernel provides only the mechanism; policy walks the delegation tree.

---

## Consequences

- Revocation is non-blocking and O(1) regardless of how many capabilities
  are bound to the lease.
- Dead capabilities linger in CSpace slots until explicitly dropped
  (`sys_cap_drop`, RFC 032). This is a resource-reuse concern, not a
  safety concern.
- Services must be notified of revocation to clean up their CSpace (best-
  effort; safety does not depend on notification delivery).
- Blocked IPC must be woken/cancelled when a lease is revoked (v0.2: RFC 034).

---

## Security Boundary Impact

At v0.1.0, lease revocation is advisory: the epoch exists in the ABI type
but is not yet connected to all use sites. RFC 033 (v0.2) makes it
enforceable everywhere.

Until RFC 033 lands, a revoked capability continues to work. This is the
most important known weakness at v0.1.x (see threat model §12).

---

## Deferred Work

- Lease epoch check at every use site: v0.2, RFC 033.
- Blocked-IPC wake/cancel on revoke: v0.2, RFC 034.
- Lifecycle revoke on task exit/fault/restart: v0.2, RFC 033 §2.10.
- Per-service notification path (LeaseRevoked / CapDropRequested): v0.2,
  RFC 032 §2.7.

---

## Related RFCs

- RFC 006 (LeaseBinding in Capability, M3/M4)
- RFC 015 (Lease Validation in IPC/Cap Paths, M4)
- RFC 033, RFC 034 (v0.2 enforcement closure)
- RFC 029 (Capability/Lease Enforcement Audit, v0.1.3)
