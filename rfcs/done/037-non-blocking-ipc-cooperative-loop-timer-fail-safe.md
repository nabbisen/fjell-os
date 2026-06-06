# RFC 037: Non-blocking IPC, cooperative loop, and timer fail-safe

**RFC ID:** 037  
**Also known as:** RFC-v0.2-007  
**Status:** Implemented (v0.2.0)
**Target version:** v0.2.0  
**Phase:** Phase 5 — Cooperative Service Separation  
**Related epics:** D (Service Plane Realization)

## Problem

Before user-space services can be split out of the inline `init`
smoke workaround (ADR-0010), there must be a service-loop pattern
that does not deadlock under cooperative scheduling.  At v0.1.0:

- `ipc_recv` always blocks.
- A spawned task that calls `ipc_recv` may yield indefinitely if no
  sender exists yet.
- A buggy service that spins in user space (e.g. forgets to call
  `sys_yield`) can starve the rest of the system.

RFC 019 (ipc_try_recv) added a sketch; this RFC promotes it to a
real syscall and adds the preemptive fail-safe.

## Proposed fix

### Non-blocking receive

```
sys_ipc_try_recv(endpoint) -> Ok(message) | Err(SysError::WouldBlock)
```

### Cooperative service loop

```rust
loop {
    match sys_ipc_try_recv(endpoint) {
        Ok(msg)                       => handle(msg),
        Err(SysError::WouldBlock)     => sys_yield(),
        Err(_)                        => break,
    }
}
```

This is the canonical service-loop shape for v0.2.  All separated
services (RFC 038) use it.

### Timer preemptive fail-safe

Even cooperative services must not be able to monopolise the hart.
The kernel timer interrupt retains a fail-safe quantum:

```
- each running task has a tick budget
- if the task exceeds the maximum quantum, the scheduler preempts it
- preemption does not require the task to call sys_yield
- repeated quantum violation is audit-visible
```

This is **not** full priority scheduling.  It is a *fail-safe* that
ensures a buggy service cannot deadlock the whole system.

Recommended initial budget: 10 ms per quantum (matching the M2
timer tick).  Tuneable per platform.

### Audit event

```
AuditKind::TaskQuantumExceeded
  - task id
  - quantum count
  - state at preemption
```

Repeated occurrences within a short window may escalate to a
service-manager fault (RFC 038 territory).

## Rationale

A pure cooperative model is too fragile for a security-oriented
system — one buggy service cannot be allowed to compromise
liveness for the rest.  A pure preemptive model is more complex
than v0.2 needs (priority inheritance, full ready-queue
discipline).  The hybrid (cooperative by default, preemptive as a
*fail-safe*) gives the simplest path that closes the liveness gap.

`try_recv` instead of timeout-bounded blocking recv is cheaper to
implement and easier to reason about; timeout semantics can be
added later if a service needs them.

## Impact

- Crates: `fjell-kernel` (new syscall, scheduler quantum logic),
  `fjell-ipc`, `fjell-abi` (new syscall number, new error),
  `fjell-syscall` (user-side helper).
- Backward compatibility: additive — adds a new syscall and a new
  scheduler behaviour, does not change existing ones.

## Test plan

### QEMU negative / liveness tests
- `NEG:SVC:SPINNING_SERVICE_PREEMPTED:PASS` —
  a service that never yields gets preempted within the quantum.
- `NEG:IPC:TRY_RECV_EMPTY_RETURNS_WOULDBLOCK:PASS`
- `NEG:IPC:TRY_RECV_WITHOUT_RIGHT:PASS`

### Acceptance gates
- A spinning service does not monopolise the hart for longer than
  the quantum.
- `try_recv` returns `WouldBlock` (not error) when no message is
  pending.
- Audit log shows quantum violations.

## Implementation notes

- Out of scope: full priority scheduling, multi-hart scheduling,
  user-controlled timeouts on `ipc_recv`.
- The quantum check belongs in the existing timer-interrupt path; no
  new interrupt source is needed.
- The audit event must not itself require scheduling work that
  could be preempted; record it as a fixed-size kernel ring entry.
