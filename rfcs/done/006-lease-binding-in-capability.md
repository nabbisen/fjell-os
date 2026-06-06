# RFC 006: LeaseBinding in Capability + lease validation in check paths

**RFC ID:** 006  
**Status:** Implemented  
_(was: Accepted, deferred to M7.1 after RFC 004)  
**Affects:** `crates/fjell-cap/src/slot.rs`, all capability check paths

## Problem (RB-04)

`LeaseTable` exists but `Capability` has no `lease_id` / `epoch_at_issue`.
`LeaseRevoke` does not invalidate any capability.  Cap grant / revoke design
cannot be made to work.

## Proposed fix

```rust
pub struct Capability {
    pub kind:      CapKind,
    pub object_id: u32,
    pub rights:    CapRights,
    pub badge:     u64,
    pub parent:    ParentRef,
    pub lease:     Option<LeaseBinding>,   // NEW
}

pub struct LeaseBinding {
    pub lease_id:       LeaseId,
    pub epoch_at_issue: LeaseEpoch,
}
```

Every capability check path must additionally verify:

```rust
if let Some(lb) = cap.lease {
    let current_epoch = lease_table.epoch(lb.lease_id);
    if current_epoch != lb.epoch_at_issue {
        return Err(SysError::InvalidCap);  // lease revoked
    }
}
```

## Impact

Large: every CSpace lookup site needs updating.  Requires LeaseTable access at
all capability check points.  Size of Capability struct increases.

## Defer condition

Implement after RFC 004 is stable and all syscall handlers have been gated.
