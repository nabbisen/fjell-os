# ADR-0009 — A/B Boot Control and Health Confirmation

**Status:** Accepted  
**Date:** 2026-05-12  
**Milestone:** M6/M7

---

## Context

Safe over-the-air updates require an A/B slot model: the currently running system
occupies one slot; a staged update occupies the other.  If the new slot fails to boot
or fails health checks, the system must be able to roll back to the last confirmed slot
without operator intervention.

---

## Decision

### Slot model

Two slots (A and B) are defined.  Each slot has a `SlotInfo` record:

```rust
pub struct SlotInfo {
    pub state:             SlotState,   // Empty | Staging | Staged | Bootable | Unbootable
    pub image_generation:  u64,
    pub remaining_tries:   u8,
    pub confirmed:         u8,          // 1 = health-confirmed
}
```

On first boot, slot A is `Bootable` (with `remaining_tries = 3` and `confirmed = 1`);
slot B is `Empty` (RFC 002 fix).

### BootControlBlock

Stored at two mirrored locations on disk (LBA 1 and LBA 33) so that a write failure to
one mirror does not destroy boot-control state.  Fields:

- `active_slot`: which slot is currently running
- `last_confirmed_slot`: last slot that passed health checks
- `candidate_slot`: `0xFF` = none, `0` = A, `1` = B
- `slot_a`, `slot_b`: `SlotInfo` records
- `crc32`: ISO 3309 CRC over all other fields (RFC 008)

Mirror selection policy: on read, choose the mirror with the higher `generation` field
that also passes `is_valid()` (magic + CRC).  This is **defined** but not yet exercised
in M7: both mirrors are written identically.

### Upgrade state machine

```
Created → Verified → Staging → Staged → CandidateSet → CandidateBoot
  → HealthCheck → Confirmed  (success path)
                → Rollback   (failure path)
```

- **Verified:** `SignedObject::verify_dev()` must return `Ok` before staging proceeds.
- **CandidateSet:** `BootControlBlock.candidate_slot` is updated and written to disk.
- **CandidateBoot:** In M7 this is simulated inline in `fjell-init` (not a real reboot).
- **HealthCheck:** `health_ok` is currently a fixed `true` constant.  A real health
  check compares running services against a `HealthTarget` struct (defined in
  `fjell-upgrade-format` but not yet connected to a runtime evaluator).
- **Rollback:** If `health_ok` is false (or `remaining_tries` reaches 0), the system
  restores `active_slot = last_confirmed_slot` and marks the candidate unbootable.

**Known limitation (RB-07):** The candidate boot is simulated; no real reboot occurs.
`health_ok = true` is hardcoded.  The mirror selection algorithm is defined but
untested.  Active-slot write rejection is defined but not enforced by `upgraded`.

### upgraded service

`fjell-upgraded` stages the new release image to the inactive slot.  In M7, inactive
slot validation (preventing writes to the active slot) is defined in the type contract
but not enforced at the kernel level; `upgraded` is a stub.

---

## Consequences

- The A/B model is structurally in place; the smoke test exercises the state transitions
  with simulated health checks.
- Real reboot-and-confirm is blocked on the preemptive scheduler (M8 prerequisite).
- Active-slot write protection will be enforced when `upgraded` is an IPC service
  and the kernel provides a write-permission capability for the inactive slot only.


## Security Boundary Impact

Boot-control write must require a capability (v0.2: RFC 031 + RFC 038). At v0.1.x the barrier is policy-level only.

## Deferred Work

- Capability-enforced bootctl IPC (v0.2: RFC 031 + 038).
- Remote rollback notification (v0.4).

## Related RFCs

- RFC 002, RFC 008, RFC 023
- RFC 038 (Service Plane Separation, v0.2)
