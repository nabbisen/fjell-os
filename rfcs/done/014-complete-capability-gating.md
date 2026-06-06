# RFC 014: Complete capability gating — require_cap() + missing syscall gates

**RFC ID:** 014  
**Status:** Implemented  
**Affects:** `crates/fjell-kernel/src/trap/syscall.rs`, `crates/fjell-cap/src/cspace.rs`

## Problem (RB-01, RB-02 from v0.0.10 review)

Two gaps remain in capability enforcement:

**Gap 1 — Missing gates:**
- `sys_task_status` has no capability check (doc says TaskControl, code has none)
- `sys_lease_revoke` has no capability check
- `sys_lease_inspect` has no capability check

**Gap 2 — caller_has_cap() ignores rights and lease:**
`caller_has_cap(kind)` returns true for any slot with a matching CapKind, without
checking `CapRights` (could be NONE) or `LeaseBinding` (could be revoked).

## Proposed fix

Replace `caller_has_cap(kind)` with:

```rust
fn require_cap(kind: CapKind, required_rights: CapRights) -> Result<(), SysError> {
    let (_, _, cap_table, _) = unsafe { get_kernel_state() };
    let lt = unsafe { get_lease_table() };
    let tidx = current_task_idx();
    let cs = cap_table.cspace(tidx).ok_or(SysError::InternalError)?;
    let found = cs.slots().iter().any(|slot| {
        if let Some(cap) = slot.cap {
            cap.kind == kind
            && cap.rights.contains(required_rights)
            && cap.check_lease(lt).is_ok()
        } else { false }
    });
    if found { Ok(()) } else { Err(SysError::PermissionDenied) }
}
```

Apply to all task/lease syscalls:

| Syscall | Required cap | Rights |
|---|---|---|
| sys_task_spawn | TaskCreate | ALL |
| sys_task_start | TaskControl | ALL |
| sys_task_status | TaskControl | INSPECT |
| sys_lease_create | LeaseAdmin | ALL |
| sys_lease_revoke | LeaseAdmin | ALL |
| sys_lease_inspect | LeaseAdmin | INSPECT |

Add `CapRights::INSPECT = 1 << 7` (already defined) to the required_rights for
read-only operations.
