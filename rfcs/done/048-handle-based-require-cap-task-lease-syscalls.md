# RFC 048: Handle-based `require_cap` for task and lease syscalls

**RFC ID:** 048
**Also known as:** RFC-v0.2-014
**Status:** Implemented
**Target version:** v0.2.10
**Phase:** Capability/syscall enforcement closure (post-v0.2.8 review)
**Closes review item:** RB-01
**Depends on:** RFC 031 (Unified capability enforcement)
**Blocks:** RFCs 049, 050, 055, 056

## Problem

`crates/fjell-kernel/src/trap/syscall.rs:72-96` defines a local
`require_cap(kind, required_rights)` helper that **scans the caller's
CSpace** for any cap matching the kind and rights, then returns Ok if found:

```rust
let found = cs.slots().iter().any(|slot| {
    slot.cap.as_ref().map_or(false, |cap| {
        cap.kind == kind
            && cap.rights.contains(required_rights)
            && cap.check_lease(lt).is_ok()
    })
});
```

This helper is used by `sys_task_spawn`, `sys_task_start`, `sys_task_status`,
`sys_task_kill`, `sys_lease_create`, `sys_lease_revoke`, and `sys_lease_inspect`.

The helper checks only **kind, rights, lease validity**.  It does not check:

- Which specific capability handle the caller intends to use
- Handle generation (so stale handles cannot be detected)
- Object scope (so a `TaskControl` cap scoped to task A can be used to
  start task B)
- Target ID match between the cap's scope and the syscall argument

Consequence: any task holding *any* `TaskControl` cap can start, inspect, or
kill **any task in the system**.  Any task holding *any* `LeaseAdmin` cap
can revoke **any lease**.  This contradicts RFC 031's invariant that all
authority-bearing syscalls validate handle + kind + rights + scope + lease.

The v0.2.0 release-gate document acknowledged this as V02-A-001.  The
external review of v0.2.8 (RB-01) classifies it as a release blocker.

## Proposed fix

Replace the scan-based helper with handle-based enforcement at every task
and lease syscall site.  The new ABI explicitly passes a capability handle
as the first argument:

```
sys_task_start  (cap_handle, task_handle, entry_pc, stack_top)
sys_task_status (cap_handle, task_handle)
sys_task_kill   (cap_handle, task_handle)
sys_lease_revoke  (cap_handle, lease_id)
sys_lease_inspect (cap_handle, lease_id)
```

For `sys_task_spawn` and `sys_lease_create`, the cap argument authorizes the
*creation* itself — no target id exists yet:

```
sys_task_spawn  (cap_handle, image_id)
sys_lease_create (cap_handle, owner_task_id)
```

### Validation steps (per call)

Every call performs the unified 7-step `require_cap` (RFC 031):

1. CSpace lookup by handle (`slot_by_handle`)
2. Generation match
3. State == Active
4. Kind == expected
5. Rights ⊇ required
6. Scope match against syscall argument (see scope table below)
7. Lease epoch active

### Scope semantics

| Syscall | Required cap | Required rights | Required scope |
|---------|--------------|-----------------|----------------|
| `sys_task_spawn` | TaskCreate | TASK_CREATE | `ObjectScope::Any` or `ObjectScope::Image(image_id)` |
| `sys_task_start` | TaskControl | TASK_START | `ObjectScope::Any` or `ObjectScope::Task(target)` |
| `sys_task_status` | TaskControl | TASK_STATUS | `ObjectScope::Any` or `ObjectScope::Task(target)` |
| `sys_task_kill` | TaskControl | TASK_KILL | `ObjectScope::Any` or `ObjectScope::Task(target)` |
| `sys_lease_create` | LeaseAdmin | LEASE_CREATE | `ObjectScope::Any` or `ObjectScope::Task(owner)` |
| `sys_lease_revoke` | LeaseAdmin | LEASE_REVOKE | `ObjectScope::Any` or `ObjectScope::Lease(lease_id)` |
| `sys_lease_inspect` | LeaseAdmin | LEASE_INSPECT | `ObjectScope::Any` or `ObjectScope::Lease(lease_id)` |

`ObjectScope::Any` remains valid — broad authority is still expressible —
but scope checking is now uniformly performed.  Specifically-scoped caps
(e.g. `Task(7)`) reject calls targeting other ids.

### `ObjectScope` additions

`crates/fjell-cap/src/scope.rs` adds two variants:

```rust
pub enum ObjectScope {
    Any,
    Image(u16),          // existing — for cap-broker manifests
    Endpoint(u32),       // existing
    MmioRegion(u32),     // existing
    DmaRegion(u32),      // existing
    Task(u16),           // NEW: scoped to TaskId
    Lease(u32),          // NEW: scoped to LeaseId
}
```

Both new variants need a one-line `matches_target(target_id) -> bool` check;
`Any` always returns true.

### ABI / syscall-number compatibility

The existing syscall numbers are preserved (`TaskSpawn=10`, `TaskStart=11`,
…, `LeaseCreate=30`, `LeaseRevoke=31`, `LeaseInspect=32`).  The argument
layout changes: arg0 was `image_id` or `task_handle` or `lease_id`; it
becomes `cap_handle`.  The previous arg0 shifts to arg1.

No syscall-number duplication.  Callers built against the old ABI will get
the wrong arguments and fail in `slot_by_handle` (the old `image_id` /
`task_handle` value won't be a valid CapHandle for the caller's CSpace).

## Rationale

**Why pass the cap handle explicitly, not use scan?** Scan was a v0.1
expedient — when no service had more than one cap of a given kind, scan
gave the same answer as handle-based lookup.  Once cap-broker delegates
narrowed caps to many services (RFC 040 direction), scan becomes wrong:
the kernel might find a broad cap when the caller intended to use a
narrow one.  Explicit handle removes the ambiguity.

**Why preserve syscall numbers?** Renumbering forces a coordinated
update across kernel, syscall wrapper crate, every service crate, and every
test.  The argument-layout change is the same shape (caller adjusts call
site) without touching dispatch tables.

**Why add `ObjectScope::Task` and `Lease` now, not later?** Without them
the new `require_cap` call would still need to broaden to `Any`.  The
ABI change is the moment to introduce them.

**Alternative considered: badge-based dispatch.** Each cap carries a
`badge` field already (RFC 040 use).  We considered checking badge for
authority decisions.  Rejected: badge is a routing aid, not a security
primitive; it has no integrity guarantee beyond what scope provides.

## Impact

### Crates affected

| Crate | Change |
|-------|--------|
| `fjell-cap` | `ObjectScope::Task` and `ObjectScope::Lease` variants; matching helpers |
| `fjell-kernel` | Replace local `require_cap` scan; rewrite 7 syscall sites |
| `fjell-syscall` | Wrapper signatures change for the 7 syscalls (extra `cap` arg) |
| `fjell-init` | Update spawn loop to pass init's pre-installed cap handles |
| `fjell-neg-test` | Update SVC test calls to use neg-test's slot 5 / slot 6 caps |
| `fjell-abi` | (no change — `SyscallNumber` enum unchanged) |

### Backward compatibility

Breaking ABI change within v0.2 hardening line.  Acceptable because:
- No external callers exist (Fjell is a closed system at v0.2)
- Every in-tree caller is updated in the same release
- The previous scan-based ABI was explicitly documented as V02-A-001
  (to-be-fixed) limitation

### Migration impact on existing caller code

```text
v0.2.9:  sys_task_start(task_handle, entry_pc, stack_top)
v0.2.10: sys_task_start(SLOT_TASK_CONTROL, task_handle, entry_pc, stack_top)
```

Init currently has `TaskCreate` in slot 28 and `TaskControl` in slot 29
(see spawn.rs).  Neg-test has them in slots 5 and 6 (added in v0.2.8).
Both callers will pass these slot handles explicitly.

## Test plan

### Host (unit tests in `fjell-cap`)

1. `ObjectScope::Task(7).matches_target(7)` → true
2. `ObjectScope::Task(7).matches_target(8)` → false
3. `ObjectScope::Any.matches_target(_)` → true
4. Same for `Lease` variants

### Host (unit tests in `fjell-kernel::cap::syscall::tests`)

5. `require_cap` succeeds for a correctly-scoped handle
6. `require_cap` fails (`PermissionDenied`) for a `Task(8)` cap used on task 7
7. `require_cap` fails (`InvalidCap`) for a generation-mismatched handle
8. `require_cap` fails (`LeaseRevoked`) after the lease is revoked

### QEMU (extended `capability` profile)

9. `NEG:CAP:WRONG_SCOPE_REJECTED:PASS` — neg-test holds a `Task(99)`-scoped
   cap, calls `sys_task_start(cap, 0)` (target task 0), receives
   `PermissionDenied`.
10. `NEG:CAP:STALE_GENERATION_REJECTED:PASS` — neg-test drops slot 5,
    re-spawns into slot 5 (new generation), then uses the *old* handle —
    receives `InvalidCap`.

These two new markers are added to RFC 042's matrix.

## Implementation notes

- `slot_by_handle` already validates generation (verified in v0.2.9 RB-03
  fix).  No new lookup primitive needed.
- The new `ObjectScope::Task`/`Lease` variants must be added to any
  `match` over `ObjectScope` in `fjell-cap` and `fjell-cap-broker`.
- Audit-record arg0 for these syscalls should carry the cap handle's slot
  index (not the raw handle) for log clarity.
- Init's pre-installed cap layout (slots 27-29) should be reviewed: with
  scope checking, init may want a narrower `TaskCreate` cap per service
  family rather than one global `TaskCreate(Any)`.  Defer that tightening
  to RFC 057 (bootctl extraction) — not required by RB-01 closure.

## Open questions

- Should `sys_task_status` for **the caller's own task** be exempt from
  cap check?  Recommendation: no — keep the rule uniform; tasks needing
  self-introspection should be given a `Task(self)`-scoped `TaskControl`
  cap at spawn time.  Document as design decision in this RFC.

- Should `LEASE_INSPECT` exist as a separate right from `LEASE_REVOKE`?
  Currently both bits are defined in `CapRights` but `LEASE_INSPECT` is
  only sketched.  Recommendation: keep them separate — read-only auditors
  may want inspect-without-revoke authority.  No additional code needed.
