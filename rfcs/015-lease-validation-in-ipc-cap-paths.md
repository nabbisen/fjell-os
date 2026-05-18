# RFC 015: Lease validation in IPC / cap_copy / cap_mint paths

**RFC ID:** 015  
**Status:** Implemented  
**Affects:** `crates/fjell-kernel/src/cap/syscall.rs`

## Problem (RB-03 from v0.0.10 review)

`Capability::check_lease()` exists (RFC 006) but is never called in:
- `check_right()` — endpoint cap access for IPC send/recv/call
- `sys_cap_copy` — source cap before copy
- `sys_cap_mint` — source cap before mint
- `sys_cap_inspect` — cap before inspect

Consequence: a revoked cap can still be used for IPC and capability operations.

## Proposed fix

### check_right() — IPC lease validation

```rust
fn check_right(...) -> Result<(), SysError> {
    let lt = unsafe { crate::get_lease_table() };
    let cap = cs.get(ep_h)?;
    if !cap.rights.contains(right) { return Err(SysError::PermissionDenied); }
    cap.check_lease(lt)?;   // RFC 015: reject revoked caps
    Ok(())
}
```

### sys_cap_copy / sys_cap_mint — source lease check

Before the mutable copy/mint operation, check the source cap's lease:

```rust
// Immutable borrow scope: validate lease
{
    let cs = ct.cspace(tidx).ok_or(...)?;
    cs.get(src)?.check_lease(lt)?;
}
// Mutable borrow: proceed with copy
let cs = ct.cspace_mut(tidx)...;
cs.copy(src, dst)?;
```

### sys_cap_inspect — lease check

Add `cap.check_lease(lt)?` before returning cap info.

## Impact

Low — additive check, no ABI change.  Revoked caps now fail IPC and derivation.
