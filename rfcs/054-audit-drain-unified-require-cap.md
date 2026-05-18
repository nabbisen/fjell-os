# RFC 054: `sys_audit_drain` unified `require_cap` with lease check

**RFC ID:** 054
**Also known as:** RFC-v0.2-020
**Status:** Implemented
**Target version:** v0.2.11
**Phase:** MMIO/DMA/audit hardening
**Closes review item:** H-02
**Depends on:** RFC 031 (Unified capability enforcement), RFC 049 (management rights pattern)

## Problem

`crates/fjell-kernel/src/trap/syscall.rs` (audit-drain section, around
line 396-405) performs an inline capability check that walks the caller's
CSpace looking for any cap whose kind is `AuditDrain` and whose rights
include `AUDIT_DRAIN`:

```rust
let cs = ct.cspace(tidx).ok_or(...)?;
let has_cap = cs.slots().iter().any(|s| {
    s.cap.as_ref().map_or(false, |c| {
        c.kind == CapKind::AuditDrain && c.rights.contains(CapRights::AUDIT_DRAIN)
    })
});
if !has_cap { err(PermissionDenied); return; }
```

This check is missing the **lease validity** step.  An auditd whose
`AuditDrain` cap has been issued a lease, and that lease has been revoked,
will still be able to drain audit records — exactly the case the lease
mechanism is supposed to handle.

The check is also the last surviving "scan-based" enforcement site outside
the task/lease syscalls covered by RFC 048.  Replacing it brings audit
drain into the same enforcement model as everything else.

## Proposed fix

Replace the inline scan with a call to `require_cap_on` (RFC 049):

```rust
let cap_h = CapHandle(tf.gpr[REG_A0] as u32);
if let Err(e) = require_cap_on(cap_h, CapKind::AuditDrain, CapRights::AUDIT_DRAIN) {
    err(tf, e);
    return;
}
```

This means `sys_audit_drain`'s ABI changes: the first argument becomes the
`AuditDrain` cap handle.  The previous arg0 (user-buffer pointer) shifts
to arg1, length to arg2.

### Revised ABI

```
sys_audit_drain(cap_handle: u32, user_buf: *mut u8, max_records: usize) -> Result<(n_copied, n_dropped), SysError>
```

Validation steps (`require_cap_on`):
1. Look up cap by handle (generation-validated)
2. Kind == `AuditDrain`
3. Rights include `AUDIT_DRAIN`
4. (Scope check skipped — audit ring has no scoping concept)
5. **Lease epoch active** (this is the H-02 fix)

If any step fails, the syscall returns the appropriate `SysError` without
draining anything.  Combined with RFC 053 (peek-copy-advance), no audit
records are consumed on failure.

### Lease binding for AuditDrain

`task/spawn.rs` already installs the AuditDrain cap with `lease: None`
(after RB-05 fix in v0.2.9).  Optional follow-up: bind the cap to a
fresh lease at spawn time so that the lease can be revoked when auditd
should be silenced:

```rust
let lease_id = lt.create_for(task_id).unwrap();
let _ = cs.install_raw(1, Capability {
    kind: CapKind::AuditDrain, object_id: 0,
    rights: CapRights::AUDIT_DRAIN,
    badge: 0, scope: ObjectScope::Any, state: CapState::Active,
    parent: None,
    lease: Some(LeaseBinding { lease_id, epoch_at_issue: 0 }),
});
```

This makes `H-02`'s fix observable in QEMU: revoking the lease
immediately disables drain authority.

## Rationale

**Why does this matter, given AuditDrain is held only by trusted
services?**  Defense-in-depth.  The lease check is part of the unified
enforcement model — exempting audit drain creates an inconsistency
that future maintainers (or compromised services) can exploit.

**Why bind to a lease at spawn time, not later?**  Lease binding is the
mechanism to disable a service's authority without dropping the cap.
A compromised auditd that holds an unleashed cap cannot be silenced
short of killing the task.  A leased cap can be silenced atomically
from outside.

**Why not use a separate `sys_audit_drain` helper rather than
`require_cap_on`?**  The whole point of RFC 031 was to have one
enforcement path.  Any custom helper accumulates inconsistencies.

## Impact

### Crates affected

| Crate | Change |
|-------|--------|
| `fjell-kernel/trap/syscall.rs` | Replace inline scan; arg layout change |
| `fjell-syscall/lib.rs` | `sys_audit_drain` wrapper takes cap handle |
| `fjell-kernel/task/spawn.rs` | Optional: bind AuditDrain to a lease |
| `fjell-auditd` (if exists) | Updated to pass slot-1 cap handle |
| `fjell-neg-test` | Updated similarly |

### Backward compatibility

ABI change — same hardening-line policy as RFC 048/049.  Only auditd and
neg-test call `sys_audit_drain` today.

### Audit trail

Failed drains record `PermissionDenied` or `LeaseRevoked` via the
unified path's existing audit hook.

## Test plan

### Host

(none specific to this RFC — `require_cap_on` is tested under RFC 049)

### QEMU

1. `NEG:AUDIT:EVIDENCE_GAP_DETECTED:PASS` — unchanged (existing).
2. **NEW** `NEG:AUDIT:LEASE_REVOKED_REJECTED:PASS` — neg-test:
   - Receives its AuditDrain cap with a lease (slot 1)
   - Calls `sys_lease_revoke` on the cap's lease
   - Calls `sys_audit_drain(slot_1, buf, n)` — gets `LeaseRevoked`.

   This marker is only meaningful if spawn.rs binds the lease (the
   optional follow-up above).  If the spawn-time lease binding is not
   implemented in v0.2.11, this marker is deferred to v0.3.

3. (No marker needed for the kind/rights checks — they're covered by the
   generic capability tests after RFC 050 makes them error-specific.)

## Implementation notes

- This is a small RFC by line count but it tightens the model in a way
  that affects every audit-bearing service.  Coordinate with
  `fjell-auditd` (if/when it lands) and `fjell-service-manager`
  (RFC 058) which may want to revoke an auditd's drain authority during
  a service restart.
- The optional lease binding requires that `lt.create_for(task_id)` is
  available at spawn time — verify ordering in `task/spawn.rs`.  Lease
  table is initialized before task spawning per current boot order.

## Open questions

- Should `AuditDrain` caps support scope at all?  Conceivably yes
  (`ObjectScope::AuditPartition(id)` for multi-tenant audit) — but
  Fjell has a single audit ring, so this is premature.  Leave
  `ObjectScope::Any` as the only meaningful scope for v0.2/v0.3.
