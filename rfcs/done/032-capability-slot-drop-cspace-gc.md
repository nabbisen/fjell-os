# RFC 032: Capability slot drop and CSpace garbage collection

**RFC ID:** 032  
**Also known as:** RFC-v0.2-002  
**Status:** Implemented (v0.2.0)
**Target version:** v0.2.0  
**Phase:** Phase 1 — Capability Enforcement Core  
**Related epics:** A (Unified Capability Enforcement), B (Lease Revocation)

## Problem

After lease revocation (RFC 031, RFC 033), a capability’s slot in the
owning task’s CSpace becomes dead but still consumes a slot.  Lazy
invalidation keeps the system *safe* — use-time checks reject the
dead capability — but a service that grants and revokes repeatedly
will eventually exhaust its CSpace.

There is currently no way for a task to release a slot it owns.

## Proposed fix

Add an explicit slot-release syscall:

```
sys_cap_drop(cap_handle)
```

After successful drop:

- The CSpace slot becomes reusable.
- The slot generation is incremented; the old handle becomes stale.
- Future lookup with the old handle fails with `GenerationMismatch`.

### CSpace slot state

```rust
pub struct CapSlot {
    pub generation: u16,
    pub state:      CapSlotState,
    pub cap:        Option<Capability>,
}

pub enum CapSlotState { Empty, Active, Dropped }

pub struct CapHandle {
    pub index:      u16,
    pub generation: u16,
}
```

Raw representation may be `u32`.

### `sys_cap_drop` ABI

```
Input:  a0 = cap_handle
Output: a0 = status
```

Errors:

```
InvalidCap, GenerationMismatch, AlreadyDropped, PermissionDenied,
Internal
```

### Drop semantics

```rust
pub fn cap_drop(task: TaskId, handle: CapHandle) -> Result<(), CapError> {
    let cspace = task_cspace_mut(task)?;
    let slot = cspace.get_slot_mut(handle.index)?;
    if slot.generation != handle.generation {
        return Err(CapError::GenerationMismatch);
    }
    if slot.state != CapSlotState::Active {
        return Err(CapError::Dropped);
    }
    slot.cap = None;
    slot.state = CapSlotState::Empty;
    slot.generation = slot.generation.wrapping_add(1);
    cspace.free_list.push(handle.index);
    audit_cap_drop(task, handle);
    Ok(())
}
```

### Interaction with revoked lease

Dropping a capability whose lease has been revoked **must succeed**.
A revoked cap is unusable, but the owner must still be able to free
the slot.  `sys_cap_drop` therefore validates only handle ownership
and generation; it does not require the lease to be active.

### Interaction with parent/child capabilities

Dropping a parent does **not** automatically drop children.  The
kernel rule is:

```
drop removes only the caller’s slot
lease revoke invalidates all lease-bound capabilities
```

Recursive policy-level revoke belongs to `cap-broker` (RFC 044).

### LeaseRevoked notification

To prevent slow CSpace fill-up, services should be notified when a
held capability has become dead:

```
cap-broker emits LeaseRevoked notification to affected service
service calls sys_cap_drop for affected caps
```

Notification channels (in v0.2 minimum, best-effort, not safety):

- direct service endpoint
- semantic-stream State/Event (`CapDropRequested`)
- service-manager health/control endpoint

Safety does not depend on notification delivery.

### CapDropRequested semantic event

```
[EVENT][Normal][Ok] Capability drop requested
Description:
  cap-broker revoked lease L and requested service S
  to drop dead capability slot C.
```

This is operational visibility, not enforcement.

## Rationale

`sys_cap_drop` is a *resource-reuse* mechanism, not a *safety*
mechanism.  The safety guarantee is provided by use-time lease
checks; this RFC adds the ability to free slots after the safety
event has happened.

Splitting safety (lease epoch) from cleanup (cap_drop) means a buggy
service that fails to call `cap_drop` cannot create a security hole
— only a resource leak that is bounded by its CSpace size.

Generation wrapping is acceptable for v0.2; CSpace sizes are small
enough that wrapping in practice means cycling through all live
slots first.  The handle-equality check (`slot.generation ==
handle.generation`) remains exact across the wrap.

## Impact

- Crates: `fjell-cap` (new function), `fjell-kernel` (syscall entry),
  `fjell-abi` (new error code, new syscall number),
  `fjell-syscall` (user-side helper).
- New audit kinds: `CapabilityDropped`, `CapabilityDropFailed`,
  `CapDropRequested`.
- Backward compatibility: additive — adds a new syscall, does not
  change existing ones.

## Test plan

### Unit tests
- Dropped capability cannot be used through `require_cap`.
- Stale handle after drop fails generation check.
- Dropping another task’s cap is impossible.

### QEMU negative tests
- `NEG:CAP:DROPPED_HANDLE:PASS`
- `NEG:CAP:STALE_AFTER_DROP:PASS`
- `NEG:CAP:DROP_REVOKED_CAP:PASS`
- `NEG:CAP:CSpace_REUSE_AFTER_DROP:PASS`
- `NEG:CAP:DROP_INVALID_HANDLE_REJECTED:PASS`

### CSpace exhaustion test

Scenario:

```
1. service receives capability grant N times
2. cap-broker revokes lease N times
3. service drops each cap
4. service receives N more grants
5. all grants succeed
```

Expected marker: `NEG:CAP:CSpace_REUSE_AFTER_DROP:PASS`.

## Implementation notes

- Out of scope: automatic recursive CSpace cleanup, kernel
  delegation-tree traversal, making garbage collection part of the
  revoke correctness model.
- Invariants the implementation must preserve:
  - `CS-001` Dropped capability cannot be used.
  - `CS-002` Stale handle after drop fails generation check.
  - `CS-003` Dropping a capability never grants authority.
  - `CS-004` Dropping a revoked capability is allowed.
  - `CS-005` Safety does not depend on `cap_drop`.
  - `CS-006` CSpace slot reuse requires generation increment.
- Documentation updates required:
  `docs/architecture/cspace.md`,
  `docs/architecture/capability-drop.md`,
  `docs/security/lazy-invalidation-and-cspace-gc.md`,
  `docs/verification/cspace-invariants.md`.
