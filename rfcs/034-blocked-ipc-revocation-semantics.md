# RFC 034: Blocked IPC revocation semantics

**RFC ID:** 034  
**Also known as:** RFC-v0.2-004  
**Status:** Proposed  
**Target version:** v0.2.0  
**Phase:** Phase 2 — Lease Revocation Semantics  
**Related epics:** B (Lease Revocation), D (Service Plane)

## Problem

After RFC 033, revoking a lease invalidates capabilities on *next
use*.  But IPC has blocking variants:

- `ipc_recv` blocks waiting for a sender.
- `ipc_call` blocks waiting for a reply.

A task blocked in either of these does not perform a "next use".
Without a wake/cancel path, a revoked lease leaves the task hanging
forever, which is indistinguishable from a deadlock.

## Proposed fix

`CallFrame` records the lease epoch observed at call time:

```rust
pub struct CallFrame {
    pub call_id:             CallId,
    pub caller:              TaskId,
    pub receiver:            TaskId,
    pub endpoint:            EndpointId,
    pub lease_id:            LeaseId,
    pub lease_epoch_at_call: u32,
    pub state:               CallState,
}
```

On `lease_revoke`, the kernel scans for and handles:

```
Blocked receiver on revoked endpoint:
  wake with LeaseRevoked

Blocked caller waiting for reply via revoked CallFrame:
  cancel CallFrame
  wake with LeaseRevoked

Late reply for a cancelled CallFrame:
  reject with InvalidCallId or LeaseRevoked
```

This is the implementation of the
`wake_or_cancel_blocked_ipc_for_lease` hook left by RFC 033 §2.4.

### Performance contract

Revoke must remain O(1) on the lease-table mutation, but the
waker scan over endpoints/CallFrames may be O(number of blocked
tasks for that lease) — *not* over all tasks in the system.  The
implementation must keep per-lease waiter lists; a naïve full scan
on every revoke is rejected.

## Rationale

Without an explicit wake path, a revoked lease either silently
deadlocks (the failure RFC 026 §IPC asserts must not happen) or the
kernel must walk every blocked task on every revoke.  Per-lease
waiter lists are the standard mechanism (linux futexes, seL4
notification queues) and match the design principle that revoke
remains O(1) on the *common* path.

Late replies are handled by rejecting the reply rather than by
keeping the CallFrame alive: the caller is already gone, and the
reply has nowhere safe to land.

## Impact

- Crates: `fjell-kernel` (IPC scheduler, lease module), `fjell-ipc`
  (CallFrame layout), `fjell-abi` (LeaseRevoked must be returnable
  from recv/call).
- Backward compatibility: changes behaviour of blocking IPC after
  revoke (was: deadlock, becomes: error return).
- Audit: revoke-induced wake/cancel events recorded.

## Test plan

### QEMU negative tests
- `NEG:IPC:BLOCKED_CALL_WAKES_ON_REVOKE:PASS`
- `NEG:IPC:BLOCKED_RECV_WAKES_ON_REVOKE:PASS`
- `NEG:IPC:LATE_REPLY_REJECTED:PASS`
- `NEG:IPC:REPLY_INVALID_CALL_ID:PASS`
- `NEG:IPC:REPLY_AFTER_REVOKE:PASS`

### Acceptance gates
- Blocked IPC does not hang after revoke.
- Revoke remains O(1) on the kernel hot path.
- Late replies are rejected, not silently accepted.

## Implementation notes

- Out of scope: timer-driven IPC timeouts (separate concept; RFC 041
  covers cooperative-service timer fail-safe).
- Per-lease waiter lists may be implemented as intrusive linked
  lists in `LeaseObject` to avoid heap allocation in `no_std`.
- The wake path must be safe to call from IRQ context if revoke is
  ever triggered by a fault handler; this constrains the locking
  strategy.
