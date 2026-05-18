//! A single capability and its containing slot.

use super::{handle::CapHandle, rights::{CapKind, CapRights}};

/// Index of a parent capability in the same CSpace (for derivation tree).
/// `None` means this is a root capability with no parent.
pub type ParentRef = Option<u16>;

/// A single capability stored in a `CapSlot`.
///
/// # Derivation tree
/// Each capability records the slot index of its parent.  Revocation walks
/// all slots looking for matching `parent` values to delete descendants.
/// For M3 with a small fixed-capacity CSpace this O(n) walk is acceptable.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Capability {
    pub kind:     CapKind,
    /// Index of the kernel object this capability refers to.
    pub object_id: u32,
    pub rights:   CapRights,
    /// Immutable badge — set at `cap_mint` time and never changed.
    pub badge:    u64,
    /// Slot index of the parent capability, if this was derived.
    pub parent:   ParentRef,
}

impl Capability {
    /// Derive a new capability from `self` with attenuated rights and badge.
    ///
    /// Returns `Err(())` if `new_rights` is not a subset of `self.rights`
    /// (invariant CAP-A).
    pub fn derive(
        &self,
        new_rights: CapRights,
        new_badge:  u64,
        self_slot:  u16,
    ) -> Result<Capability, ()> {
        if !new_rights.is_subset_of(self.rights) {
            return Err(());
        }
        Ok(Capability {
            kind:      self.kind,
            object_id: self.object_id,
            rights:    new_rights,
            badge:     new_badge,
            parent:    Some(self_slot),
        })
    }
}

/// One slot in a CSpace.
///
/// Each slot has an independent generation counter so that recycled slots
/// are not confused with old handles (invariant CAP-C).
#[derive(Clone, Copy, Debug, Default)]
pub struct CapSlot {
    /// Generation counter, bumped on `cap_delete` when the slot is recycled.
    pub generation: u16,
    /// The capability stored in this slot, or `None` if the slot is empty.
    pub cap: Option<Capability>,
}

impl CapSlot {
    pub const fn empty() -> Self {
        CapSlot { generation: 0, cap: None }
    }

    /// Does this slot hold a capability?
    pub fn is_occupied(&self) -> bool {
        self.cap.is_some()
    }

    /// Install a capability into this (must be empty) slot.
    pub fn install(&mut self, cap: Capability) -> Result<CapHandle, ()> {
        if self.cap.is_some() {
            return Err(());
        }
        self.cap = Some(cap);
        Ok(CapHandle::new(0 /* filled in by CSpace */, self.generation))
    }

    /// Remove the capability, bumping the generation counter.
    pub fn clear(&mut self) {
        self.cap = None;
        self.generation = self.generation.wrapping_add(1);
    }
}
