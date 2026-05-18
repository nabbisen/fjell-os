//! A single capability and its containing slot.

use super::{handle::CapHandle, rights::{CapKind, CapRights}};
use fjell_abi::lease::{LeaseEpoch, LeaseId};

// ── RFC 006: lease binding ────────────────────────────────────────────────────

/// Lease binding attached to a delegated capability.
///
/// When the lease is revoked, `LeaseTable::check_active(lease_id, epoch_at_issue)`
/// returns `Err` and the capability is treated as invalid.
///
/// Bootstrap capabilities (slots 28–30 in init's CSpace) carry `lease: None`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LeaseBinding {
    pub lease_id:       LeaseId,
    pub epoch_at_issue: LeaseEpoch,
}

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
    /// Optional lease binding (RFC 006).
    ///
    /// `None`  — capability is not lease-bound (e.g. bootstrap caps).
    /// `Some`  — capability is invalidated when the lease epoch advances.
    pub lease:    Option<LeaseBinding>,
}

impl Capability {
    /// Validate the lease binding against `lease_table` (RFC 006).
    ///
    /// Returns `Ok(())` for unbound caps.  Returns `Err(PermissionDenied)` if
    /// the lease has been revoked (epoch mismatch) or is no longer active.
    /// Validate the lease binding.  Pass a `&dyn LeaseChecker`.
    ///
    /// Returns `Ok(())` for unbound caps or when the lease is still active.
    pub fn check_lease(
        &self,
        checker: &dyn LeaseChecker,
    ) -> Result<(), fjell_abi::error::SysError> {
        if let Some(lb) = self.lease {
            checker.check_active(lb.lease_id, lb.epoch_at_issue)?;
        }
        Ok(())
    }
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
            lease:     self.lease,  // RFC 006: derived cap inherits the lease binding
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

// ── RFC 006: lease validation trait ──────────────────────────────────────────

/// Abstraction over the kernel lease table for cap-independent validation.
///
/// The kernel passes a `&dyn LeaseChecker` (or concrete `LeaseTable` ref) to
/// capability check paths that need to verify lease liveness.
pub trait LeaseChecker {
    fn check_active(
        &self,
        id:           fjell_abi::lease::LeaseId,
        epoch_issued: fjell_abi::lease::LeaseEpoch,
    ) -> Result<(), fjell_abi::error::SysError>;
}

/// Type alias used in `Capability::check_lease` — a reference to any
/// `LeaseChecker` implementor.
pub mod lease {
    pub type LeaseRef<'a> = dyn super::LeaseChecker + 'a;
}
