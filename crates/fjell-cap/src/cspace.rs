//! Per-task fixed-capacity capability space (CSpace).
//!
//! # Invariants
//! - CAP-A: derived capability rights ⊆ source rights  (enforced in `mint`)
//! - CAP-B: no cycles in the derivation tree            (structural: parent
//!           always has lower slot index than child in simple allocator)
//! - CAP-C: stale handles (wrong generation) are rejected by `slot_by_handle`

use super::handle::CapHandle;
use super::rights::{CapError, CapKind, CapRights, CapState, ObjectScope};
use super::slot::{CapSlot, CapSlotState, Capability};
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

    /// Find the first empty slot index.
    fn alloc_slot_idx(&self) -> Result<usize, SysError> {
        self.slots.iter().position(|s| s.state == CapSlotState::Empty)
            .ok_or(SysError::NoMemory)
    }

    /// Build a handle for slot `idx` using its current generation.
    fn handle_for(&self, idx: usize) -> CapHandle {
        CapHandle::new(idx as u16, self.slots[idx].generation)
    }

    // ── Slot accessors used by enforcement.rs ─────────────────────────────

    /// Resolve a handle to an immutable slot reference, validating generation.
    ///
    /// Returns `Err(CapError::InvalidHandle)` if out of range.
    /// Returns `Err(CapError::GenerationMismatch)` on a stale handle.
    pub fn slot_by_handle(&self, h: CapHandle) -> Result<&CapSlot, CapError> {
        let idx = h.slot() as usize;
        let slot = self.slots.get(idx).ok_or(CapError::InvalidHandle)?;
        if slot.generation != h.generation() {
            return Err(CapError::GenerationMismatch);
        }
        Ok(slot)
    }

    /// Resolve a handle to a mutable slot reference, validating generation.
    pub fn slot_by_handle_mut(&mut self, h: CapHandle) -> Result<&mut CapSlot, CapError> {
        let idx = h.slot() as usize;
        if idx >= CSPACE_SLOTS {
            return Err(CapError::InvalidHandle);
        }
        if self.slots[idx].generation != h.generation() {
            return Err(CapError::GenerationMismatch);
        }
        Ok(&mut self.slots[idx])
    }

    // ── Legacy resolve helpers (used by copy / mint / delete / revoke) ────

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
        if idx >= CSPACE_SLOTS { return Err(SysError::InvalidCap); }
        if self.slots[idx].generation != h.generation() {
            return Err(SysError::InvalidCap);
        }
        Ok((idx, &mut self.slots[idx]))
    }

    // ── Public API ────────────────────────────────────────────────────────

    /// Install a root capability with `ObjectScope::Any` (bootstrap / test use).
    pub fn install_root(
        &mut self,
        kind:      CapKind,
        object_id: u32,
        rights:    CapRights,
    ) -> Result<CapHandle, SysError> {
        self.install_root_scoped(kind, object_id, rights, ObjectScope::Any)
            .map_err(|_| SysError::NoMemory)
    }

    /// Install a root capability with an explicit scope.
    ///
    /// Used by the kernel during task creation and by the enforcement tests.
    pub fn install_root_scoped(
        &mut self,
        kind:      CapKind,
        object_id: u32,
        rights:    CapRights,
        scope:     ObjectScope,
    ) -> Result<CapHandle, ()> {
        let idx = self.alloc_slot_idx().map_err(|_| ())?;
        let cap = Capability {
            kind,
            state:     CapState::Active,
            object_id,
            rights,
            badge:     0,
            scope,
            parent:    None,
            lease:     None,
        };
        self.slots[idx].install(idx as u16, cap)
    }

    /// Install a capability into a specific slot (bootstrap use only).
    pub fn install_raw(&mut self, slot: usize, cap: Capability) -> Result<(), ()> {
        let s = self.slots.get_mut(slot).ok_or(())?;
        if s.state == CapSlotState::Active { return Err(()); }
        s.cap   = Some(cap);
        s.state = CapSlotState::Active;
        Ok(())
    }

    /// Get an immutable reference to the capability in slot `h`.
    pub fn get(&self, h: CapHandle) -> Result<&Capability, SysError> {
        let (_, slot) = self.resolve(h)?;
        slot.cap.as_ref().ok_or(SysError::SlotEmpty)
    }

    /// `cap_copy`: copy capability `src` into `dst_slot`.
    pub fn copy(
        &mut self,
        src:      CapHandle,
        dst_slot: usize,
    ) -> Result<CapHandle, SysError> {
        let (src_idx, _) = self.resolve(src)?;
        let cap = self.slots[src_idx].cap.as_ref().ok_or(SysError::SlotEmpty)?.clone();

        if dst_slot >= CSPACE_SLOTS { return Err(SysError::InvalidArg); }
        if self.slots[dst_slot].state == CapSlotState::Active {
            return Err(SysError::SlotOccupied);
        }

        let mut new_cap = cap;
        new_cap.parent = None;  // copied cap has no parent link
        self.slots[dst_slot].cap   = Some(new_cap);
        self.slots[dst_slot].state = CapSlotState::Active;
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
        if dst_slot >= CSPACE_SLOTS { return Err(SysError::InvalidArg); }
        if self.slots[dst_slot].state == CapSlotState::Active {
            return Err(SysError::SlotOccupied);
        }

        // Inherit parent scope; badge change is allowed.
        let derived = source_cap.derive(new_rights, new_badge, ObjectScope::Any, src_idx as u16)
            .map_err(|_| SysError::RightsExceed)?;
        self.slots[dst_slot].cap   = Some(derived);
        self.slots[dst_slot].state = CapSlotState::Active;
        Ok(self.handle_for(dst_slot))
    }

    /// `cap_delete`: remove the capability in slot `h`.
    ///
    /// The slot is cleared and the generation bumped.  The kernel object is
    /// not destroyed (other CSpace references may exist).
    pub fn delete(&mut self, h: CapHandle) -> Result<(), SysError> {
        let (idx, _) = self.resolve_mut(h)?;
        if self.slots[idx].state != CapSlotState::Active {
            return Err(SysError::SlotEmpty);
        }
        self.slots[idx].clear();
        Ok(())
    }

    /// `cap_revoke`: delete all descendants of `h` (children, grandchildren…).
    ///
    /// The capability at `h` itself is NOT deleted.
    pub fn revoke(&mut self, h: CapHandle) -> Result<(), SysError> {
        let (root_idx, _) = self.resolve(h)?;
        self.slots[root_idx].cap.as_ref().ok_or(SysError::SlotEmpty)?;

        loop {
            let mut deleted_any = false;
            for i in 0..CSPACE_SLOTS {
                if self.slots[i].state == CapSlotState::Active {
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

    fn is_descendant_of(
        slots:        &[CapSlot; CSPACE_SLOTS],
        candidate:    usize,
        ancestor_idx: usize,
    ) -> bool {
        let mut current = slots[candidate].cap.as_ref().and_then(|c| c.parent);
        for _ in 0..CSPACE_SLOTS {
            match current {
                None                                        => return false,
                Some(p) if p as usize == ancestor_idx      => return true,
                Some(p) => {
                    current = slots[p as usize].cap.as_ref().and_then(|c| c.parent);
                }
            }
        }
        false
    }

    /// `cap_inspect`: return basic information about a capability slot.
    pub fn inspect(&self, h: CapHandle) -> Result<(CapKind, CapRights, u64), SysError> {
        let cap = self.get(h)?;
        Ok((cap.kind, cap.rights, cap.badge))
    }

    /// Read-only view of all slots (for kernel scan paths).
    pub fn slots(&self) -> &[CapSlot; CSPACE_SLOTS] {
        &self.slots
    }

    /// Mutable view of all slots (test/bootstrap use only).
    pub fn slots_mut(&mut self) -> &mut [CapSlot; CSPACE_SLOTS] {
        &mut self.slots
    }
}
