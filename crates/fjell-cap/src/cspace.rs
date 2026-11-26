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
    /// Grants refused since the last [`CSpace::finish_grants`] (RFC-0.49-001 R2). A grant that cannot be
    /// installed is **recorded**, never lost: see [`CSpace::grant`].
    grant_failures: u8,
    /// The slot of the first refused grant, for the message. Meaningful only while `grant_failures > 0`.
    first_failed_slot: u8,
    /// Grants installed since the last [`CSpace::finish_grants`]; compared with the occupied-slot count there.
    grants_installed: u8,
}

/// Why a grant section did not end clean ([`CSpace::finish_grants`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantError {
    /// `n` grants were refused (the slot was already `Active`, or out of range); the first was into `first_slot`.
    Refused { first_slot: usize, n: usize },
    /// Every grant installed, but the CSpace holds **more capabilities than were granted**: something was already in it.
    /// On a reused CSpace that is the previous occupant's authority, which nothing in the new image asked for.
    Leftover { occupied: usize, granted: usize },
}

impl CSpace {
    pub const fn new() -> Self {
        CSpace {
            slots: [CapSlot::empty(); CSPACE_SLOTS],
            grant_failures: 0,
            first_failed_slot: 0,
            grants_installed: 0,
        }
    }

    /// Install a **boot grant** into `slot` and, if it cannot be installed, **remember that it could not** (RFC-0.49-001).
    ///
    /// This replaces the `let _ = cs.install_raw(..)` form the kernel's 55 grant sites used. `install_raw` refuses a slot
    /// that is already `Active`, and a discarded refusal is invisible on a fresh CSpace only because nothing has ever
    /// collided; **in a reused CSpace the refusal is the whole story** -- the new task would hold the previous occupant's
    /// capability in that slot and its own grant would silently not exist. The spawner calls [`CSpace::finish_grants`]
    /// once after its grant section and **refuses the spawn** if any grant was refused.
    pub fn grant(&mut self, slot: usize, cap: Capability) {
        if self.install_raw(slot, cap).is_err() {
            if self.grant_failures == 0 {
                self.first_failed_slot = slot.min(u8::MAX as usize) as u8;
            }
            self.grant_failures = self.grant_failures.saturating_add(1);
        } else {
            self.grants_installed = self.grants_installed.saturating_add(1);
        }
    }

    /// End of a grant section. `Ok` only if **every** [`CSpace::grant`] installed **and the CSpace holds exactly the
    /// capabilities that were granted** -- nothing more. The second half is what makes reuse safe *by invariant* rather than by
    /// a test someone remembers to run: a CSpace that still holds a previous occupant's capability in a slot the new image does
    /// not grant is refused here, at every spawn, not found by a scenario.
    ///
    /// Clears the record either way, so one section's failure is never charged to the next.
    pub fn finish_grants(&mut self) -> Result<(), GrantError> {
        let (n, first, installed) = (
            self.grant_failures as usize,
            self.first_failed_slot as usize,
            self.grants_installed as usize,
        );
        self.grant_failures = 0;
        self.first_failed_slot = 0;
        self.grants_installed = 0;
        if n > 0 {
            return Err(GrantError::Refused {
                first_slot: first,
                n,
            });
        }
        let occupied = self.slots.iter().filter(|s| s.is_occupied()).count();
        if occupied != installed {
            return Err(GrantError::Leftover {
                occupied,
                granted: installed,
            });
        }
        Ok(())
    }

    /// Empty every slot, **bumping each slot's generation** (RFC-0.49-001 D4). Used when a task's slot is destroyed so the
    /// next occupant starts with no capability of its predecessor's, and a handle the predecessor held -- or handed to
    /// anyone -- names nothing. Also clears the grant record.
    pub fn reset(&mut self) {
        for s in self.slots.iter_mut() {
            if s.state != CapSlotState::Empty || s.cap.is_some() {
                s.clear();
            }
        }
        self.grant_failures = 0;
        self.first_failed_slot = 0;
        self.grants_installed = 0;
    }

    // ── Internal helpers ──────────────────────────────────────────────────

    /// Find the first empty slot index.
    fn alloc_slot_idx(&self) -> Result<usize, SysError> {
        self.slots
            .iter()
            .position(|s| s.state == CapSlotState::Empty)
            .ok_or(SysError::NoMemory)
    }

    /// Install a capability into the first available empty slot.
    /// Returns the handle of the installed slot, or `Err(CSpaceFull)`.
    pub fn install_any(&mut self, cap: Capability) -> Result<CapHandle, SysError> {
        use fjell_abi::error::SysError;
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if slot.state == CapSlotState::Empty {
                slot.cap = Some(cap);
                slot.state = CapSlotState::Active;
                let h = CapHandle::new(i as u16, slot.generation);
                return Ok(h);
            }
        }
        Err(SysError::NoMemory)
    }

    /// Whether `install_any` would find a slot: the one predicate `sys_cap_install` asks
    /// **before** it writes (RFC-0.45-002 R3 f), so a full CSpace refuses with nothing touched.
    /// It is `install_any`'s own scan, so the two cannot disagree (a host test holds them to it).
    pub fn has_free_slot(&self) -> bool {
        self.slots.iter().any(|s| s.state == CapSlotState::Empty)
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
        if idx >= CSPACE_SLOTS {
            return Err(SysError::InvalidCap);
        }
        if self.slots[idx].generation != h.generation() {
            return Err(SysError::InvalidCap);
        }
        Ok((idx, &mut self.slots[idx]))
    }

    // ── Public API ────────────────────────────────────────────────────────

    /// Install a root capability with `ObjectScope::Any` (bootstrap / test use).
    pub fn install_root(
        &mut self,
        kind: CapKind,
        object_id: u32,
        rights: CapRights,
    ) -> Result<CapHandle, SysError> {
        self.install_root_scoped(kind, object_id, rights, ObjectScope::Any)
            .map_err(|_| SysError::NoMemory)
    }

    /// Install a root capability with an explicit scope.
    ///
    /// Used by the kernel during task creation and by the enforcement tests.
    pub fn install_root_scoped(
        &mut self,
        kind: CapKind,
        object_id: u32,
        rights: CapRights,
        scope: ObjectScope,
    ) -> Result<CapHandle, ()> {
        let idx = self.alloc_slot_idx().map_err(|_| ())?;
        let cap = Capability {
            kind,
            state: CapState::Active,
            object_id,
            rights,
            badge: 0,
            scope,
            parent: None,
            lease: None,
        };
        self.slots[idx].install(idx as u16, cap)
    }

    /// Install a capability into a specific slot (bootstrap use only).
    pub fn install_raw(&mut self, slot: usize, cap: Capability) -> Result<(), ()> {
        let s = self.slots.get_mut(slot).ok_or(())?;
        if s.state == CapSlotState::Active {
            return Err(());
        }
        s.cap = Some(cap);
        s.state = CapSlotState::Active;
        Ok(())
    }

    /// Get an immutable reference to the capability in slot `h`.
    pub fn get(&self, h: CapHandle) -> Result<&Capability, SysError> {
        let (_, slot) = self.resolve(h)?;
        slot.cap.as_ref().ok_or(SysError::SlotEmpty)
    }

    /// `cap_copy`: copy capability `src` into `dst_slot`.
    ///
    /// # Parent tracking (RFC-v0.7.4-003 / C-M-09 — intentional design)
    ///
    /// `new_cap.parent = None` is deliberate. Kernel-local `cap_revoke` does NOT
    /// cascade across copies; this is by design:
    ///
    ///   - Copies are **independent authorities**. Kernel revoke only clears the
    ///     single slot being revoked; other copies of the same object remain valid.
    ///   - **Recursive revocation is done via lease revocation**, not cap-parent
    ///     links. Any copied cap that shares the same `LeaseBinding` as the
    ///     original is automatically invalidated when that lease is revoked. A
    ///     binding is **set once** (RFC-0.38-004) — `sys_cap_bind_lease` refuses a
    ///     capability that already has one, even to the same lease — so this holds
    ///     without qualification: the holder cannot move a capability off its lease
    ///     before the lease is revoked.
    ///
    /// The copy is a clone with only `parent` cleared, so **it inherits its
    /// source's binding**. If your code expects recursive revocation across copies:
    ///   1. If the source is already bound, the copy is already governed by the
    ///      same lease and nothing further is needed — binding it again would be
    ///      refused.
    ///   2. If the source is not bound, bind each capability once via
    ///      `sys_cap_bind_lease` (the caller must hold `LeaseAdmin` with
    ///      `LEASE_CREATE`, and the lease must be `Active`; anything else is
    ///      refused). To move a capability to another lease, drop it and copy again
    ///      from an unbound source — there is deliberately no unbind.
    ///   3. Use `sys_lease_revoke` rather than `sys_cap_revoke`.
    ///
    /// See RFC-v0.7.4-003 (referred to as ADR-v0.7.4-003 in older text).
    pub fn copy(&mut self, src: CapHandle, dst_slot: usize) -> Result<CapHandle, SysError> {
        let (src_idx, _) = self.resolve(src)?;
        let cap = self.slots[src_idx]
            .cap
            .as_ref()
            .ok_or(SysError::SlotEmpty)?
            .clone();

        if dst_slot >= CSPACE_SLOTS {
            return Err(SysError::InvalidArg);
        }
        if self.slots[dst_slot].state == CapSlotState::Active {
            return Err(SysError::SlotOccupied);
        }

        let mut new_cap = cap;
        // Intentional: parent = None. See doc comment above for the rationale.
        // Lease-based revocation is the correct mechanism for cascading revoke.
        new_cap.parent = None;
        self.slots[dst_slot].cap = Some(new_cap);
        self.slots[dst_slot].state = CapSlotState::Active;
        Ok(self.handle_for(dst_slot))
    }

    /// `cap_mint`: derive a capability with attenuated rights and/or new badge.
    ///
    /// Enforces CAP-A: new_rights ⊆ source.rights.
    pub fn mint(
        &mut self,
        src: CapHandle,
        dst_slot: usize,
        new_rights: CapRights,
        new_badge: u64,
    ) -> Result<CapHandle, SysError> {
        let (src_idx, _) = self.resolve(src)?;
        let source_cap = self.slots[src_idx]
            .cap
            .as_ref()
            .ok_or(SysError::SlotEmpty)?
            .clone();

        if !new_rights.is_subset_of(source_cap.rights) {
            return Err(SysError::RightsExceed);
        }
        if dst_slot >= CSPACE_SLOTS {
            return Err(SysError::InvalidArg);
        }
        if self.slots[dst_slot].state == CapSlotState::Active {
            return Err(SysError::SlotOccupied);
        }

        // Inherit parent scope; badge change is allowed.
        let derived = source_cap
            .derive(new_rights, new_badge, ObjectScope::Any, src_idx as u16)
            .map_err(|_| SysError::RightsExceed)?;
        self.slots[dst_slot].cap = Some(derived);
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
        self.slots[root_idx]
            .cap
            .as_ref()
            .ok_or(SysError::SlotEmpty)?;

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
            if !deleted_any {
                break;
            }
        }
        Ok(())
    }

    fn is_descendant_of(
        slots: &[CapSlot; CSPACE_SLOTS],
        candidate: usize,
        ancestor_idx: usize,
    ) -> bool {
        let mut current = slots[candidate].cap.as_ref().and_then(|c| c.parent);
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

    /// Iterate over all occupied capability slots, yielding `&Capability`.
    /// Used by the kernel to scan CSpaces for authority checks (RFC-v0.7.4-003).
    pub fn iter_occupied(&self) -> impl Iterator<Item = &Capability> {
        self.slots.iter().filter_map(|slot| {
            if slot.state == crate::slot::CapSlotState::Active {
                slot.cap.as_ref()
            } else {
                None
            }
        })
    }
}

#[cfg(test)]
mod grant_tests {
    use super::*;
    use crate::rights::{CapKind, CapRights, CapState, ObjectScope};

    fn cap() -> Capability {
        Capability {
            kind: CapKind::Endpoint,
            object_id: 0,
            rights: CapRights::SEND,
            badge: 0,
            scope: ObjectScope::Any,
            state: CapState::Active,
            parent: None,
            lease: None,
        }
    }

    #[test]
    fn a_grant_into_a_free_slot_installs_and_finishes_ok() {
        let mut cs = CSpace::new();
        cs.grant(3, cap());
        assert!(cs.slots()[3].is_occupied());
        assert_eq!(cs.finish_grants(), Ok(()));
    }

    #[test]
    fn a_refused_grant_is_remembered_not_lost() {
        let mut cs = CSpace::new();
        cs.grant(3, cap());
        cs.grant(3, cap()); // the slot is Active: install_raw refuses
        cs.grant(70, cap()); // out of range: refused
        assert_eq!(
            cs.finish_grants(),
            Err(GrantError::Refused {
                first_slot: 3,
                n: 2
            }),
            "first refused slot, and the count"
        );
        cs.reset();
        assert_eq!(
            cs.finish_grants(),
            Ok(()),
            "the record is cleared, so one section is not charged to the next"
        );
    }

    #[test]
    fn reset_empties_every_slot_and_bumps_each_generation() {
        let mut cs = CSpace::new();
        cs.grant(0, cap());
        cs.grant(5, cap());
        let h0 = CapHandle::new(0, cs.slots()[0].generation);
        assert!(cs.get(h0).is_ok());
        cs.reset();
        assert!(cs.slots().iter().all(|s| !s.is_occupied()));
        assert!(
            cs.get(h0).is_err(),
            "the predecessor's handle names nothing"
        );
        // the new occupant's grant into the same slot now installs, and the stale handle still does not reach it
        cs.grant(0, cap());
        assert_eq!(cs.finish_grants(), Ok(()));
        assert!(
            cs.get(h0).is_err(),
            "a stale handle must not reach the new occupant's capability"
        );
    }

    #[test]
    fn without_reset_a_reused_cspace_refuses_the_new_grant_which_is_the_hazard() {
        let mut cs = CSpace::new();
        cs.grant(0, cap()); // the predecessor's
        cs.grant(0, cap()); // the new occupant's, into the same slot
        assert!(cs.finish_grants().is_err());
    }

    #[test]
    fn a_capability_nobody_granted_is_a_leftover_and_refuses_the_section() {
        let mut cs = CSpace::new();
        // the previous occupant's capability, installed outside the new section's grants
        cs.install_raw(9, cap()).unwrap();
        cs.grant(0, cap());
        assert_eq!(
            cs.finish_grants(),
            Err(GrantError::Leftover {
                occupied: 2,
                granted: 1
            }),
            "every grant installed, and the space still holds something nobody granted"
        );
    }

    #[test]
    fn reset_then_grants_is_a_clean_section_and_the_record_does_not_leak_between_sections() {
        let mut cs = CSpace::new();
        cs.install_raw(9, cap()).unwrap(); // a predecessor's
        cs.reset();
        cs.grant(0, cap());
        cs.grant(1, cap());
        assert_eq!(cs.finish_grants(), Ok(()));
        // a second, empty section on the same space: 2 occupied, 0 granted -> not clean, which is the right answer
        assert_eq!(
            cs.finish_grants(),
            Err(GrantError::Leftover {
                occupied: 2,
                granted: 0
            })
        );
    }
}
