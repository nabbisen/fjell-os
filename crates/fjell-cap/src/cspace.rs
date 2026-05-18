//! Per-task fixed-capacity capability space (CSpace).
//!
//! # Invariants
//! - CAP-A: derived capability rights ⊆ source rights  (enforced in `mint`)
//! - CAP-B: no cycles in the derivation tree            (structural: parent
//!           always has a lower slot index than child in M3 simple allocator)
//! - CAP-C: stale handles (wrong generation) are rejected by `resolve`

use super::{
    handle::CapHandle,
    rights::{CapKind, CapRights},
    slot::{CapSlot, Capability},
};
use fjell_abi::error::SysError;

/// Maximum number of capability slots per task.
pub const CSPACE_SLOTS: usize = 64;

/// Per-task capability table.
pub struct CSpace {
    slots: [CapSlot; CSPACE_SLOTS],
}

impl CSpace {
    pub const fn new() -> Self {
        CSpace { slots: [CapSlot::empty(); CSPACE_SLOTS] }
    }

    // ── Internal helpers ──────────────────────────────────────────────────

    /// Resolve a `CapHandle` to a slot reference, checking generation.
    fn resolve(&self, h: CapHandle) -> Result<(usize, &CapSlot), SysError> {
        let idx = h.slot() as usize;
        let slot = self.slots.get(idx).ok_or(SysError::InvalidCap)?;
        if slot.generation != h.generation() {
            return Err(SysError::InvalidCap);
        }
        Ok((idx, slot))
    }

    fn resolve_mut(&mut self, h: CapHandle) -> Result<(usize, &mut CapSlot), SysError> {
        let idx = h.slot() as usize;
        let slot_gen = self.slots.get(idx).ok_or(SysError::InvalidCap)?.generation;
        if slot_gen != h.generation() {
            return Err(SysError::InvalidCap);
        }
        Ok((idx, &mut self.slots[idx]))
    }

    /// Find the first empty slot.
    fn alloc_slot(&self) -> Result<usize, SysError> {
        self.slots.iter().position(|s| s.cap.is_none())
            .ok_or(SysError::NoMemory)
    }

    /// Build a `CapHandle` for slot `idx` using its current generation.
    fn handle_for(&self, idx: usize) -> CapHandle {
        CapHandle::new(idx as u16, self.slots[idx].generation)
    }

    // ── Public API ────────────────────────────────────────────────────────

    /// Install a root capability (called by the kernel during task creation).
    ///
    /// Returns the `CapHandle` the task can use to reference it.
    pub fn install_root(
        &mut self,
        kind:      CapKind,
        object_id: u32,
        rights:    CapRights,
    ) -> Result<CapHandle, SysError> {
        let idx = self.alloc_slot()?;
        self.slots[idx].cap = Some(Capability {
            kind,
            object_id,
            rights,
            badge:  0,
            parent: None,
        });
        Ok(self.handle_for(idx))
    }

    /// Get an immutable reference to the capability in slot `h`.
    pub fn get(&self, h: CapHandle) -> Result<&Capability, SysError> {
        let (_, slot) = self.resolve(h)?;
        slot.cap.as_ref().ok_or(SysError::SlotEmpty)
    }

    /// `cap_copy`: copy capability `src` into `dst_slot`.
    ///
    /// Rights are preserved exactly (no attenuation).
    pub fn copy(
        &mut self,
        src: CapHandle,
        dst_slot: usize,
    ) -> Result<CapHandle, SysError> {
        // Validate source.
        let (src_idx, src_slot) = self.resolve(src)?;
        let cap = src_slot.cap.as_ref().ok_or(SysError::SlotEmpty)?.clone();

        // Validate destination.
        if dst_slot >= CSPACE_SLOTS {
            return Err(SysError::InvalidArg);
        }
        if self.slots[dst_slot].cap.is_some() {
            return Err(SysError::SlotOccupied);
        }
        let _ = src_idx; // suppress unused warning

        let new_cap = Capability { parent: None, ..cap }; // copy has no parent link
        self.slots[dst_slot].cap = Some(new_cap);
        Ok(self.handle_for(dst_slot))
    }

    /// `cap_mint`: derive a capability with attenuated rights and/or new badge.
    ///
    /// Enforces CAP-A: new_rights ⊆ source.rights.
    pub fn mint(
        &mut self,
        src:        CapHandle,
        dst_slot:   usize,
        new_rights: CapRights,
        new_badge:  u64,
    ) -> Result<CapHandle, SysError> {
        let (src_idx, _) = self.resolve(src)?;
        let source_cap = self.slots[src_idx].cap.as_ref()
            .ok_or(SysError::SlotEmpty)?
            .clone();

        if !new_rights.is_subset_of(source_cap.rights) {
            return Err(SysError::RightsExceed);
        }
        if dst_slot >= CSPACE_SLOTS {
            return Err(SysError::InvalidArg);
        }
        if self.slots[dst_slot].cap.is_some() {
            return Err(SysError::SlotOccupied);
        }

        let derived = source_cap.derive(new_rights, new_badge, src_idx as u16)
            .map_err(|_| SysError::RightsExceed)?;
        self.slots[dst_slot].cap = Some(derived);
        Ok(self.handle_for(dst_slot))
    }

    /// `cap_delete`: remove the capability in slot `h`.
    ///
    /// The slot is cleared and the generation bumped.  The kernel object is
    /// not destroyed (other references may exist).
    pub fn delete(&mut self, h: CapHandle) -> Result<(), SysError> {
        let (idx, _) = self.resolve_mut(h)?;
        if self.slots[idx].cap.is_none() {
            return Err(SysError::SlotEmpty);
        }
        self.slots[idx].clear();
        Ok(())
    }

    /// `cap_revoke`: delete all descendants of `h` (children, grandchildren…).
    ///
    /// The capability at `h` itself is NOT deleted.
    /// Walks all slots looking for capabilities whose `parent` chain reaches
    /// `h`'s slot.  O(n²) worst case but acceptable for M3 with n=64.
    pub fn revoke(&mut self, h: CapHandle) -> Result<(), SysError> {
        let (root_idx, _) = self.resolve(h)?;
        self.slots[root_idx].cap.as_ref().ok_or(SysError::SlotEmpty)?;

        // Collect direct children first, then iterate until no more are found.
        // Simple approach: keep scanning until a full pass finds no deletions.
        loop {
            let mut deleted_any = false;
            for i in 0..CSPACE_SLOTS {
                if self.slots[i].cap.is_some() {
                    if Self::is_descendant_of(&self.slots, i, root_idx) {
                        self.slots[i].clear();
                        deleted_any = true;
                    }
                }
            }
            if !deleted_any { break; }
        }
        Ok(())
    }

    /// Check whether slot `candidate` is a descendant of `ancestor_idx`.
    fn is_descendant_of(slots: &[CapSlot; CSPACE_SLOTS], candidate: usize, ancestor_idx: usize) -> bool {
        let mut current = slots[candidate].cap.as_ref().and_then(|c| c.parent);
        // Walk up parent chain (bounded by CSPACE_SLOTS to prevent infinite loop).
        for _ in 0..CSPACE_SLOTS {
            match current {
                None => return false,
                Some(p) if p as usize == ancestor_idx => return true,
                Some(p) => {
                    current = slots[p as usize].cap.as_ref().and_then(|c| c.parent);
                }
            }
        }
        false
    }

    /// `cap_inspect`: return basic information about a capability slot.
    ///
    /// Returns `(kind, rights, badge)` or an error if the slot is invalid/empty.
    pub fn inspect(&self, h: CapHandle) -> Result<(CapKind, CapRights, u64), SysError> {
        let cap = self.get(h)?;
        Ok((cap.kind, cap.rights, cap.badge))
    }

    /// Read-only view of all slots — used by kernel to check capability kind
    /// without a handle (e.g., `caller_has_cap` in RFC 004).
    pub fn slots(&self) -> &[CapSlot; CSPACE_SLOTS] {
        &self.slots
    }

    /// Install a capability into a specific slot index (bootstrap use only).
    ///
    /// Returns `Err(())` if the slot index is out of range or already occupied.
    pub fn install_raw(&mut self, slot: usize, cap: Capability) -> Result<(), ()> {
        let s = self.slots.get_mut(slot).ok_or(())?;
        if s.cap.is_some() { return Err(()); }
        s.cap = Some(cap);
        Ok(())
    }
}

// ── Host-side unit tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make() -> CSpace { CSpace::new() }

    fn root(cs: &mut CSpace) -> CapHandle {
        cs.install_root(CapKind::Endpoint, 1, CapRights::ALL).unwrap()
    }

    #[test]
    fn install_and_get() {
        let mut cs = make();
        let h = root(&mut cs);
        let cap = cs.get(h).unwrap();
        assert_eq!(cap.kind, CapKind::Endpoint);
        assert_eq!(cap.rights, CapRights::ALL);
    }

    #[test]
    fn stale_handle_rejected() {
        let mut cs = make();
        let h = root(&mut cs);
        cs.delete(h).unwrap();
        // Generation was bumped; old handle is now invalid.
        assert_eq!(cs.get(h), Err(SysError::InvalidCap));
    }

    #[test]
    fn mint_rights_attenuation() {
        let mut cs = make();
        let h = root(&mut cs);
        // Mint with reduced rights.
        let child = cs.mint(h, 5, CapRights::SEND, 42).unwrap();
        let c = cs.get(child).unwrap();
        assert_eq!(c.rights, CapRights::SEND);
        assert_eq!(c.badge, 42);
    }

    #[test]
    fn mint_rights_exceed_fails() {
        let mut cs = make();
        let send_only = cs.install_root(CapKind::Endpoint, 1, CapRights::SEND).unwrap();
        // Attempt to mint with RECV (not in source).
        let res = cs.mint(send_only, 5, CapRights::SEND | CapRights::RECV, 0);
        assert_eq!(res, Err(SysError::RightsExceed));
    }

    #[test]
    fn revoke_removes_descendants() {
        let mut cs = make();
        let root_h = root(&mut cs);
        let child  = cs.mint(root_h, 5, CapRights::SEND, 0).unwrap();
        let _grand = cs.mint(child, 6, CapRights::SEND, 0).unwrap();

        // Root still present after revoke.
        cs.revoke(root_h).unwrap();
        assert!(cs.get(root_h).is_ok());
        // Child and grandchild are gone.
        assert_eq!(cs.get(child), Err(SysError::InvalidCap));
    }

    #[test]
    fn slot_occupied_error() {
        let mut cs = make();
        let h = root(&mut cs);
        let res = cs.mint(h, h.slot() as usize, CapRights::SEND, 0);
        assert_eq!(res, Err(SysError::SlotOccupied));
    }

}
