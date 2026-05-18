//! A single capability and its containing CSpace slot (RFC 031 §2.4, RFC 032 §2.1).

use super::handle::CapHandle;
use super::rights::{CapKind, CapRights, CapState, ObjectScope};
use fjell_abi::lease::{LeaseEpoch, LeaseId};

// ── Lease binding ─────────────────────────────────────────────────────────────

/// Lease binding attached to a delegated capability (RFC 006 / RFC 033).
///
/// When the owning lease is revoked, `check_lease()` returns `Err(LeaseRevoked)`
/// because the lease table's current epoch no longer matches `epoch_at_issue`.
///
/// Bootstrap capabilities carry `lease: None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeaseBinding {
    pub lease_id:       LeaseId,
    pub epoch_at_issue: LeaseEpoch,
}

// ── Capability ────────────────────────────────────────────────────────────────

/// A single capability stored in a `CapSlot` (RFC 031 §2.4).
///
/// # Derivation tree
/// Each capability records the slot index of its parent.  The kernel's
/// `cap_revoke` walks all slots for matching `parent` values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Capability {
    pub kind:      CapKind,
    pub state:     CapState,
    /// Index of the kernel object this capability refers to.
    pub object_id: u32,
    pub rights:    CapRights,
    /// Immutable badge — set at `cap_mint` time.
    pub badge:     u64,
    /// Object scope that limits the target of operations (RFC 031 §2.3).
    pub scope:     ObjectScope,
    /// Slot index of the parent capability (for derivation tree), if any.
    pub parent:    Option<u16>,
    /// Optional lease binding (RFC 006 / RFC 033).
    /// `None` for bootstrap capabilities.
    pub lease:     Option<LeaseBinding>,
}

impl Capability {
    /// Validate the lease binding against the given `checker`.
    ///
    /// Returns `Ok(())` for unbound caps or when the lease is still active.
    /// Returns `Err(CapError::LeaseRevoked)` on epoch mismatch or revoked state.
    pub fn check_lease(
        &self,
        checker: &dyn LeaseChecker,
    ) -> Result<(), super::rights::CapError> {
        if let Some(lb) = self.lease {
            checker.check_active(lb.lease_id, lb.epoch_at_issue)?;
        }
        Ok(())
    }

    /// Derive a new capability from `self` with attenuated rights and optional badge.
    ///
    /// Enforces invariant CAP-A: `new_rights ⊆ self.rights`.
    /// Enforces scope narrowing: child scope must be `Any` (inherits parent scope)
    /// or equal to the parent scope — widening is rejected.
    ///
    /// Returns `Err(())` on rights amplification.
    pub fn derive(
        &self,
        new_rights:    CapRights,
        new_badge:     u64,
        new_scope:     ObjectScope,
        self_slot:     u16,
    ) -> Result<Capability, ()> {
        if !new_rights.is_subset_of(self.rights) {
            return Err(());
        }
        // Scope narrowing: child may be `Any` (inherits) or equal to parent;
        // it may not be *broader* than the parent.
        let effective_scope = match new_scope {
            ObjectScope::Any => self.scope,   // inherit parent scope
            s if s == self.scope => s,         // same scope is fine
            _ => return Err(()),               // would widen: reject
        };
        Ok(Capability {
            kind:      self.kind,
            state:     CapState::Active,
            object_id: self.object_id,
            rights:    new_rights,
            badge:     new_badge,
            scope:     effective_scope,
            parent:    Some(self_slot),
            lease:     self.lease,   // derived cap inherits the lease binding
        })
    }
}

// ── CapSlotState ──────────────────────────────────────────────────────────────

/// Lifecycle state of a CSpace slot (RFC 032 §2.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CapSlotState {
    /// The slot holds no capability and is available for allocation.
    #[default]
    Empty,
    /// The slot holds an active capability.
    Active,
    /// The slot was explicitly dropped via `cap_drop`; reserved for
    /// deferred-cleanup pathways.  At v0.2.0, `cap_drop` immediately
    /// transitions to `Empty` rather than lingering in this state.
    Dropped,
}

// ── CapSlot ───────────────────────────────────────────────────────────────────

/// One slot in a CSpace (RFC 031 §2.4, RFC 032 §2.1).
///
/// # Generation counter
/// The generation is bumped on every `clear()`.  An old `CapHandle` whose
/// generation does not match the current slot generation is rejected.
/// Invariant CAP-C.
#[derive(Clone, Copy, Debug)]
pub struct CapSlot {
    pub generation: u16,
    pub state:      CapSlotState,
    pub cap:        Option<Capability>,
}

impl Default for CapSlot {
    fn default() -> Self { Self::empty() }
}

impl CapSlot {
    pub const fn empty() -> Self {
        CapSlot { generation: 0, state: CapSlotState::Empty, cap: None }
    }

    /// Is this slot occupied by a live capability?
    pub fn is_occupied(&self) -> bool {
        self.state == CapSlotState::Active && self.cap.is_some()
    }

    /// Install a capability into this slot.
    ///
    /// Returns the `CapHandle` for the installed slot.
    /// Fails if the slot is currently occupied.
    pub fn install(&mut self, slot_idx: u16, cap: Capability) -> Result<CapHandle, ()> {
        if self.state == CapSlotState::Active {
            return Err(());
        }
        self.cap   = Some(cap);
        self.state = CapSlotState::Active;
        Ok(CapHandle::new(slot_idx, self.generation))
    }

    /// Clear the slot, bumping the generation.  Used by `cap_delete` / `cap_revoke`.
    ///
    /// Transitions: Active → Empty, generation++.
    pub fn clear(&mut self) {
        self.cap       = None;
        self.state     = CapSlotState::Empty;
        self.generation = self.generation.wrapping_add(1);
    }
}

// ── LeaseChecker trait ────────────────────────────────────────────────────────

/// Abstraction over the kernel lease table for capability validation.
///
/// The kernel passes a `&dyn LeaseChecker` (or a concrete `LeaseTable` ref) to
/// `require_cap()` so the pure-logic `fjell-cap` crate can remain arch-free.
pub trait LeaseChecker {
    /// Return `Ok(())` if the lease identified by `id` is still active with
    /// epoch `epoch_issued`, otherwise `Err(CapError::LeaseRevoked)`.
    fn check_active(
        &self,
        id:           LeaseId,
        epoch_issued: LeaseEpoch,
    ) -> Result<(), super::rights::CapError>;
}

/// A no-op `LeaseChecker` that always succeeds.
///
/// Used in unit tests and for bootstrap capabilities where no lease is bound.
pub struct NoLease;

impl LeaseChecker for NoLease {
    fn check_active(
        &self,
        _id:    LeaseId,
        _epoch: LeaseEpoch,
    ) -> Result<(), super::rights::CapError> {
        Ok(())
    }
}

/// A `LeaseChecker` that always rejects (simulates a revoked lease).
///
/// Used in unit tests for RFC 031 §2.12 negative cases.
pub struct AlwaysRevoked;

impl LeaseChecker for AlwaysRevoked {
    fn check_active(
        &self,
        _id:    LeaseId,
        _epoch: LeaseEpoch,
    ) -> Result<(), super::rights::CapError> {
        Err(super::rights::CapError::LeaseRevoked)
    }
}

/// Module alias for backward compatibility with code that imports `lease::LeaseRef`.
pub mod lease {
    pub type LeaseRef<'a> = dyn super::LeaseChecker + 'a;
}
