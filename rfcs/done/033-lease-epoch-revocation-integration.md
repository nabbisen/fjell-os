# RFC 033: Lease epoch revocation integration

**RFC ID:** 033  
**Also known as:** RFC-v0.2-003  
**Status:** Implemented (v0.2.0)
**Target version:** v0.2.0  
**Phase:** Phase 2 — Lease Revocation Semantics  
**Related epics:** A (Unified Capability Enforcement), B (Lease Revocation), F (cap-broker)

## Problem

`LeaseObject`, `LeaseBinding`, and `LeaseTable` exist as partial
infrastructure at v0.1.0 but are not consulted by the syscall and
IPC paths.  Revoking a lease has no observable effect on existing
capabilities — they continue to work until the slot is dropped.

This means:

- `lease_revoke` is currently advisory.
- Recursive revocation cannot land in `cap-broker` (RFC 044) until
  kernel revocation is real.
- The "Capability OS boundary closure" claim of v0.2 depends on this
  RFC.

## Proposed fix

Connect lease epoch revocation to every capability use.  After this
RFC:

```
If a lease is revoked, every capability bound to the old lease
epoch must fail on use.
```

### Lease types

```rust
pub struct LeaseId       { pub index: u16, pub generation: u16 }
pub struct LeaseObject   { pub id: LeaseId, pub state: LeaseState,
                           pub epoch: u32, pub owner: TaskId,
                           pub flags: LeaseFlags }
pub enum   LeaseState    { Empty, Active, Revoked }
pub struct LeaseBinding  { pub lease_id: LeaseId,
                           pub epoch_at_issue: u32 }
```

### Lease table

```rust
pub struct LeaseTable {
    entries:   [LeaseSlot; MAX_LEASES],
    free_list: FixedVec<LeaseIndex, MAX_LEASES>,
}
pub struct LeaseSlot {
    pub generation: u16,
    pub object:     Option<LeaseObject>,
}
```

### Lease creation

```rust
pub fn lease_create(owner: TaskId, flags: LeaseFlags)
    -> Result<LeaseId, LeaseError>
{
    let slot = allocate_slot()?;
    let id   = LeaseId { index: slot.index, generation: slot.generation };
    slot.object = Some(LeaseObject {
        id, state: LeaseState::Active, epoch: 1, owner, flags
    });
    audit_lease_created(owner, id);
    Ok(id)
}
```

Epoch starts at `1`; `0` is reserved for invalid/default diagnostics.

### Lease revocation — O(1) kernel path

```rust
pub fn lease_revoke(caller: TaskId, lease_id: LeaseId)
    -> Result<(), LeaseError>
{
    let lease = lookup_mut(lease_id)?;
    lease.epoch = lease.epoch.wrapping_add(1);
    lease.state = LeaseState::Revoked;
    audit_lease_revoked(caller, lease_id, lease.epoch);
    wake_or_cancel_blocked_ipc_for_lease(lease_id);
    Ok(())
}
```

The epoch increment is the entire revocation operation.  Walking
delegation trees, notifying services, and cleaning up CSpace slots
are *not* the kernel’s job and never block revoke.

`wake_or_cancel_blocked_ipc_for_lease` is specified by RFC 034
(Blocked IPC Revocation Semantics); this RFC may add the hook but
leaves the full waker behaviour there.

### Lease check (called by `require_cap`)

```rust
pub fn check_lease(binding: LeaseBinding) -> Result<(), CapError> {
    let lease = lease_table.lookup(binding.lease_id)?;
    if lease.state != LeaseState::Active {
        return Err(CapError::LeaseRevoked);
    }
    if lease.epoch != binding.epoch_at_issue {
        return Err(CapError::LeaseRevoked);
    }
    Ok(())
}
```

### Lease-bound capability grants

Normal service grants must carry:

```rust
lease: Some(LeaseBinding {
    lease_id,
    epoch_at_issue: current_lease_epoch,
})
```

`lease: None` is allowed only for:

- the initial `init` bootstrap authority,
- the initial `cap-broker` bootstrap authority,
- kernel-internal capabilities not exposed to normal services.

### Source-capability lease checks

The following operations must check the source capability’s lease
before proceeding:

```
cap_copy, cap_mint, cap_inspect,
ipc_send, ipc_recv, ipc_try_recv, ipc_call, ipc_reply,
mmio_map,
dma_alloc / dma_use,
task_start / task_status (if using task caps),
audit_drain
```

### Special case: `sys_cap_drop`

`sys_cap_drop` must **not** require the lease to be active (see also
RFC 032).  A task must be able to remove dead capabilities; the
generation-stale check is sufficient to prevent cross-task drops.

### Lifecycle revoke

When a service exits, faults, or is restarted:

```
service-manager or cap-broker requests revoke of leases owned by
that service
kernel invalidates lease epochs
blocked IPC is woken/cancelled
affected services are notified where possible
```

The kernel may also perform emergency lifecycle revoke for
task-owned kernel resources.

### cap-broker delegation tree

`cap-broker` keeps:

```rust
pub struct DelegationRecord {
    pub parent:   Option<CapRef>,
    pub child:    CapRef,
    pub lease_id: LeaseId,
    pub from:     ServiceId,
    pub to:       ServiceId,
    pub rights:   CapRights,
    pub reason:   GrantReason,
}
```

Recursive revoke is implemented as:

```
1. cap-broker computes affected delegation subtree
2. cap-broker extracts affected lease ids
3. cap-broker calls sys_lease_revoke for each lease
4. kernel increments epochs
5. affected capabilities fail on next use
```

The kernel itself never walks the tree.  This keeps kernel
revocation O(1) regardless of how deep the delegation graph is.

## Rationale

The epoch-stamp pattern is the cheapest known mechanism that
provides O(1) revocation with O(1) use-time checking.  The
alternative — walking every capability table on revoke — would
either block under load or require background sweeping with
unbounded latency.

Putting recursive revoke in `cap-broker` instead of the kernel is a
deliberate split: the kernel provides the mechanism (lease
invalidation), user-space provides the policy (which leases to
revoke).  This matches Fjell OS’s design principle of
mechanism-only kernels.

`cap_drop` is exempt from the active-lease requirement because the
opposite policy would prevent legitimate cleanup after revocation
and force the system to leak slots.

## Impact

- Crates: `fjell-kernel` (lease module, hooks in `require_cap`),
  `fjell-cap` (binding types), `fjell-abi` (new error code).
- New audit kinds: `LeaseCreated`, `LeaseRevoked`,
  `LeaseCheckFailed`, `LeaseExpired`.
- Backward compatibility: **changes** the success path of every
  authority-bearing syscall — a revoked capability that used to
  succeed will now fail.  This is the intended behaviour.

## Test plan

### Unit tests
- `lease_create` returns an active lease.
- `lease_revoke` increments the epoch and sets state to Revoked.
- `check_lease` accepts a matching epoch.
- `check_lease` rejects an old epoch.
- `check_lease` rejects the Revoked state.
- `cap` with no lease follows bootstrap-only rules.

### QEMU negative tests
- `NEG:LEASE:REVOKED_CAP_SEND_REJECTED:PASS`
- `NEG:LEASE:REVOKED_CAP_CALL_REJECTED:PASS`
- `NEG:LEASE:REVOKED_CAP_MINT_REJECTED:PASS`
- `NEG:LEASE:REVOKED_CAP_COPY_REJECTED:PASS`
- `NEG:LEASE:REVOKED_CAP_DROP_ALLOWED:PASS`
- `NEG:LEASE:BOOTSTRAP_UNLEASED_CAP_RESTRICTED:PASS`

## Implementation notes

- Out of scope: recursive revocation in kernel, wall-clock expiration,
  remote policy update, per-service policy tree in kernel,
  full cap-broker (RFC 044).
- Invariants the implementation must preserve:
  - `LEASE-001` Lease revoke is O(1) in kernel.
  - `LEASE-002` Revoked lease invalidates all lease-bound capabilities.
  - `LEASE-003` Recursive revocation is not implemented in kernel.
  - `LEASE-004` cap-broker owns policy-level revoke trees.
  - `LEASE-005` `cap_drop` remains possible for revoked capabilities.
  - `LEASE-006` Lease epoch mismatch always rejects capability use.
- Documentation updates required:
  `docs/architecture/lease-epoch-revocation.md`,
  `docs/security/revocation-model.md`,
  `docs/verification/lease-invariants.md`,
  `docs/development/lease-negative-tests.md`.
