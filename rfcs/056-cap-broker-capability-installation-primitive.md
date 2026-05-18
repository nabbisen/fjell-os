# RFC 056: cap-broker capability installation primitive

**RFC ID:** 056
**Also known as:** RFC-v0.2-022
**Status:** Implemented
**Target version:** v0.2.12
**Phase:** Service separation + release-gate close
**Closes review item:** RB-11 (installation half)
**Depends on:** RFC 040 (cap-broker bootstrap), RFC 055 (sender identity)

## Problem

`crates/fjell-cap-broker/src/main.rs:398-420` makes a policy decision and
records a `DelegationRecord`, then replies with a tag:

```rust
match evaluate(requester_id, resource_class, requested_rights) {
    Verdict::Granted(_) => {
        // ... create lease, append delegation record ...
        sys_ipc_reply(tags::CAP_GRANTED);
    }
    Verdict::Denied => {
        sys_ipc_reply(tags::CAP_DENIED);
    }
}
```

No actual capability is installed into the requester's CSpace.  The
requester receives only a reply tag.  Subsequent operations attempting to
use the "granted" authority fail — there is no cap to use.

cap-broker is therefore a **policy evaluator**, not a capability broker.
The release-gate goal "policy decides → requester receives a usable cap"
is not met.

## Proposed fix

### New kernel primitive: `sys_cap_install`

```
sys_cap_install(
    install_cap_handle,    // CapInstall — held by cap-broker only
    target_task_id,        // who receives the cap
    template_kind,         // CapKind enum value
    template_object_id,    // u32
    template_rights,       // u64
    template_scope,        // packed ObjectScope
    lease_id,              // Some(LeaseId) for leased grants; 0 for unleashed
) -> Result<CapHandle, SysError>
```

The kernel:

1. Validates `install_cap_handle` via `require_cap_on(install_cap_handle,
   CapKind::CapInstall, CapRights::CAP_INSTALL)`.  This authority is
   granted to cap-broker only (slot 1 at spawn time).
2. Finds the target task's CSpace.
3. Allocates an empty slot in the target's CSpace.
4. Installs a `Capability` with the supplied fields.
5. If `lease_id` is non-zero, binds the cap's lease using the lease
   table's current epoch.
6. Returns the new `CapHandle` to cap-broker.

cap-broker then sends this handle back to the requester in the reply
payload:

```rust
sys_ipc_reply_words(tags::CAP_GRANTED, new_handle.0 as usize, 0, 0);
```

(Requires `sys_ipc_reply_words` — see Implementation Notes.)

### `CapKind::CapInstall`

New cap kind, added to `fjell-cap::CapKind`:

```rust
pub enum CapKind {
    // existing kinds ...
    CapInstall,    // RFC 056: authority to install caps into other CSpaces
}
```

New right: `CapRights::CAP_INSTALL = 1 << 26` (existing rights go up to
bit 25).  Granted at spawn time only to `CAP_BROKER` (and to `INIT` for
the bootstrap phase, then revoked once cap-broker takes over —
see RFC 057).

### Updated cap-broker grant flow

```rust
match evaluate(sender.image_id.0, resource_class, requested_rights) {
    Verdict::Granted(granted_rights) => {
        let lease = lt.create_for(target_task)?;
        let template = build_template(resource_class, granted_rights);
        match sys_cap_install(
            SLOT_INSTALL_CAP,
            sender.task_id,
            template.kind as u32,
            template.object_id,
            template.rights.bits(),
            template.scope.encode(),
            lease.0,
        ) {
            Ok(new_handle) => {
                delegation_log.append(DelegationRecord { ... });
                sys_ipc_reply_words(tags::CAP_GRANTED, new_handle.0 as usize, 0, 0);
            }
            Err(_) => {
                lt.revoke(lease);  // release the lease
                sys_ipc_reply(tags::CAP_BROKER_ERROR);
            }
        }
    }
    Verdict::Denied => {
        sys_ipc_reply(tags::CAP_DENIED);
    }
}
```

### Requester side

The requester (caller of `sys_ipc_call_words(broker_ep, CAP_REQUEST, ...)`)
gets the reply with payload word 0 = new cap handle:

```rust
let (reply, h0, _, _) = sys_ipc_call_words_with_reply(broker_ep, CAP_REQUEST, ...);
match reply & 0xFFFF {
    CAP_GRANTED => {
        let new_handle = CapHandle(h0 as u32);
        // Use new_handle for the operation.
    }
    CAP_DENIED => { ... }
}
```

## Rationale

**Why a new syscall instead of writing CSpace directly from cap-broker?**
cap-broker is a userspace service.  Modifying another task's CSpace
requires kernel mediation.  The `sys_cap_install` primitive is the
mediation point.

**Why one global `CapInstall` cap instead of per-target?**  Per-target
would require a separate kernel primitive to *issue* CapInstall caps,
recursing the problem.  A single root authority (granted at spawn time)
short-circuits this.

**Why is `CAP_INSTALL` right bit 26 (separate)?**  It is a meta-right:
authority to *create* authority.  Mixing it with operational rights
would let any service holding `ALL` install caps into others.  Distinct
bit keeps the privilege separable.

**What stops a malicious cap-broker?**  Nothing in the kernel — that's
the whole point of having a broker as a trusted base service.  RFC 057
ensures `INIT` holds the cap until `BOOTSTRAP_COMPLETE`, at which point
the kernel revokes init's CapInstall cap (handled by RFC 057's
typestate machine).  After bootstrap, only cap-broker has it.

**Why send the new handle to the requester?**  Without it, the
requester has no way to know which slot received the cap.  cap-broker
returning the handle eliminates a guess-the-slot dance.

## Impact

### Crates affected

| Crate | Change |
|-------|--------|
| `fjell-cap` | `CapKind::CapInstall`, `CapRights::CAP_INSTALL` |
| `fjell-kernel/cap/syscall.rs` | `sys_cap_install` implementation |
| `fjell-kernel/task/spawn.rs` | Install `CapInstall` cap into INIT (slot N) and CAP_BROKER (slot N) |
| `fjell-syscall` | `sys_cap_install` wrapper, `sys_ipc_reply_words` (or replace `sys_ipc_reply`) |
| `fjell-cap-broker` | Grant flow rewrite |
| `fjell-service-api` | New tags: `CAP_BROKER_ERROR` (if not already), `CAP_GRANTED_WITH_HANDLE` (or reuse `CAP_GRANTED` with payload) |

### Backward compatibility

The cap-broker reply payload changes.  All requesters need updating —
currently `fjell-neg-test` is the only requester in-tree.

### Audit trail

New audit event `AuditKindInternal::CapInstall` records:
- `arg0 = target_task_id`
- `arg1 = installed CapHandle (packed)`
- `result = 0` or error code

This is the closing piece for evidence of policy decisions producing
real authority changes.

## Test plan

### Host

1. `CapKind::CapInstall` round-trips through serialization
2. `CapRights::CAP_INSTALL` bit position doesn't collide with existing bits

### Host (kernel unit tests)

3. `sys_cap_install` with valid `CapInstall` cap installs into target
   CSpace and returns the handle
4. With missing `CAP_INSTALL` right → `PermissionDenied`
5. With non-existent target task → `BadState`
6. When target CSpace is full → `ResourceExhausted`

### QEMU

7. `NEG:POLICY:DEFAULT_DENY:PASS` — unchanged
8. **NEW** `NEG:POLICY:GRANT_INSTALLS_USABLE_CAP:PASS` — neg-test
   requests an `Endpoint` cap that policy allows; receives a handle in
   the reply payload; calls `sys_ipc_send` using the new handle and
   succeeds.  Proves end-to-end policy-to-usable-authority path.
9. **NEW** `NEG:CAP_INSTALL:WITHOUT_AUTHORITY_REJECTED:PASS` — neg-test
   directly calls `sys_cap_install` (no CapInstall cap) → `PermissionDenied`.

## Implementation notes

- The lease created for the grant is owned by `target_task_id` so that
  task exit revokes the cap.  cap-broker holds the LeaseId so that
  policy revocation is also possible.
- `sys_ipc_reply_words` should accept up to 4 reply payload words.  If
  fjell-syscall doesn't have this yet, RFC 056 adds it.
- The `template_scope` argument needs an encoding scheme.  Recommend
  packing: high byte = scope discriminant, low 24 bits = scope value
  (task_id, region_id, etc.).  Codify in `fjell-cap::scope::pack` /
  `unpack` helpers.
- Cap-broker should record the installed `CapHandle` in its
  `DelegationRecord` for later targeted revocation.

## Open questions

- Should the kernel verify that `template_rights ⊆ install_cap_owner's
  authority`?  Currently cap-broker has full `ALL` rights for its
  CapInstall cap, so this check is trivial — but RFC 057 may want to
  narrow cap-broker's authority by family.  Defer to RFC 057.
- Should cap-broker keep its own copy of issued caps (for revocation)?
  The lease-based revocation handles this without copies, but a
  delegation log indexed by (target, handle) would aid `sys_cap_revoke`
  ergonomics.  Defer to v0.3.
