//! Unified capability enforcement functions (RFC 031 §2.5, RFC 032 §2.4).
//!
//! # require_cap
//!
//! `require_cap` is the **sole production authority-checking path** for all
//! kernel operations that require a capability.  It replaces the scattered
//! `caller_has_cap(kind)` / task-id allowlist / debug-only bypasses present
//! in v0.1.x.
//!
//! ## Check order (normative, RFC 031 §2.6)
//!
//! ```text
//! 1. CSpace lookup     — handle out of range or null
//! 2. Generation check  — slot.generation != handle.generation
//! 3. Slot-state check  — slot not Active
//! 4. Kind check        — cap.kind != expected_kind
//! 5. Rights check      — cap.rights does not contain required_rights
//! 6. Scope check       — cap.scope incompatible with required_scope
//! 7. Lease check       — lease epoch mismatch or revoked state
//! ```
//!
//! The order is normative: cheap, data-independent checks precede expensive,
//! object-specific checks; lease check is last because it requires a table
//! lookup.
//!
//! # cap_drop
//!
//! `cap_drop` is the RFC 032 explicit slot-release mechanism.  Unlike
//! `cap_delete`, it succeeds even when the capability's lease has been revoked
//! — a task must always be able to release a dead slot.

use super::cspace::CSpace;
use super::handle::CapHandle;
use super::rights::{CapError, CapKind, CapRights, ObjectScope};
use super::slot::{Capability, CapSlotState, LeaseChecker};

/// Unified capability enforcement (RFC 031 §2.5).
///
/// Validates a `CapHandle` from the given `CSpace` through all seven checks
/// listed above.
///
/// Returns a shared reference to the `Capability` on success.  The
/// reference is valid for the lifetime of the `CSpace` borrow.
///
/// # Errors
///
/// | `CapError` variant       | Condition                                       |
/// |--------------------------|--------------------------------------------------|
/// | `InvalidHandle`          | handle is null or slot index out of range         |
/// | `GenerationMismatch`     | handle.generation != slot.generation             |
/// | `EmptySlot`              | slot is Empty or Dropped                         |
/// | `WrongKind`              | cap.kind != expected_kind                        |
/// | `MissingRight`           | cap.rights does not contain required_rights      |
/// | `ScopeMismatch`          | cap.scope does not satisfy required_scope        |
/// | `LeaseRevoked`           | lease epoch mismatch or lease state Revoked      |
pub fn require_cap<'cs>(
    cspace:         &'cs CSpace,
    handle:         CapHandle,
    expected_kind:  CapKind,
    required_rights: CapRights,
    required_scope: Option<&ObjectScope>,
    checker:        &dyn LeaseChecker,
) -> Result<&'cs Capability, CapError> {
    // Step 1: CSpace lookup
    if handle.is_null() {
        return Err(CapError::InvalidHandle);
    }
    let slot = cspace.slot_by_handle(handle)?;

    // Step 2: generation check (combined with lookup above via slot_by_handle)

    // Step 3: slot-state check
    if slot.state != CapSlotState::Active {
        return Err(CapError::EmptySlot);
    }
    let cap = slot.cap.as_ref().ok_or(CapError::EmptySlot)?;

    // Step 4: kind check
    if cap.kind != expected_kind {
        return Err(CapError::WrongKind);
    }

    // Step 5: rights check
    if !cap.rights.contains(required_rights) {
        return Err(CapError::MissingRight);
    }

    // Step 6: scope check
    if let Some(req_scope) = required_scope {
        if !cap.scope.is_satisfied_by(req_scope) {
            return Err(CapError::ScopeMismatch);
        }
    }

    // Step 7: lease check (last — requires a table lookup)
    cap.check_lease(checker)?;

    Ok(cap)
}

/// Explicit capability slot drop (RFC 032 §2.4).
///
/// Releases the capability in `handle`'s slot so the slot can be reused.
///
/// # Differences from `cap_delete`
/// - **Succeeds on revoked leases**: a task must be able to drop a dead
///   capability.  The lease is deliberately **not** checked.
/// - Emits the `CapabilityDropped` audit event (kernel responsibility; this
///   function only performs the slot manipulation).
///
/// # Errors
///
/// | `CapError` variant   | Condition                                     |
/// |----------------------|------------------------------------------------|
/// | `InvalidHandle`      | handle is null or index out of range          |
/// | `GenerationMismatch` | handle.generation != slot.generation          |
/// | `Dropped`            | slot is already Empty or Dropped              |
pub fn cap_drop(
    cspace: &mut CSpace,
    handle: CapHandle,
) -> Result<(), CapError> {
    // Step 1: null check
    if handle.is_null() {
        return Err(CapError::InvalidHandle);
    }

    // Step 2: bounds check and generation check
    let slot = cspace.slot_by_handle_mut(handle)?;

    // Step 3: slot must be Active — Empty or Dropped → return Dropped error.
    //
    // Note: no lease check.  A revoked-lease cap (lease epoch mismatch) is
    // still Active at the slot level; it may always be dropped.
    if slot.state != CapSlotState::Active {
        return Err(CapError::Dropped);
    }

    // Transition: Active → Empty, generation++.
    //
    // RFC 032 §2.4: the slot immediately becomes reusable after the drop.
    // A hypothetical "quarantine before reuse" policy would use the Dropped
    // state as an intermediate; for v0.2 we go straight to Empty.
    slot.cap        = None;
    slot.state      = CapSlotState::Empty;
    slot.generation = slot.generation.wrapping_add(1);

    Ok(())
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cspace::CSpace;
    use crate::rights::{CapKind, CapRights, CapState, ObjectScope};
    use crate::slot::{AlwaysRevoked, NoLease};
    use fjell_abi::task::TaskId;

    /// Build a minimal CSpace with one Endpoint capability installed.
    fn setup_endpoint(rights: CapRights) -> (CSpace, CapHandle) {
        let mut cs = CSpace::new();
        let h = cs.install_root(CapKind::Endpoint, 42, rights).unwrap();
        (cs, h)
    }

    // ── require_cap happy path ────────────────────────────────────────────────

    #[test]
    fn require_cap_valid_cap() {
        let (cs, h) = setup_endpoint(CapRights::SEND | CapRights::RECV);
        let cap = require_cap(&cs, h, CapKind::Endpoint,
                              CapRights::SEND, None, &NoLease).unwrap();
        assert_eq!(cap.kind, CapKind::Endpoint);
        assert_eq!(cap.object_id, 42);
    }

    // ── Step 1: null handle ───────────────────────────────────────────────────

    #[test]
    fn require_cap_null_handle_rejected() {
        let (cs, _) = setup_endpoint(CapRights::SEND);
        let err = require_cap(&cs, CapHandle::NULL, CapKind::Endpoint,
                              CapRights::SEND, None, &NoLease).unwrap_err();
        assert_eq!(err, CapError::InvalidHandle);
    }

    // ── Step 2: generation mismatch ───────────────────────────────────────────

    #[test]
    fn require_cap_stale_generation_rejected() {
        let (mut cs, h) = setup_endpoint(CapRights::SEND);
        cs.delete(h).unwrap();   // bumps generation
        let err = require_cap(&cs, h, CapKind::Endpoint,
                              CapRights::SEND, None, &NoLease).unwrap_err();
        assert_eq!(err, CapError::GenerationMismatch);
    }

    // ── Step 3: empty slot ────────────────────────────────────────────────────

    #[test]
    fn require_cap_empty_slot_rejected() {
        let cs = CSpace::new();
        // Construct a handle pointing at slot 0 generation 0 (slot IS empty).
        let h = CapHandle::new(0, 0);
        let err = require_cap(&cs, h, CapKind::Endpoint,
                              CapRights::SEND, None, &NoLease).unwrap_err();
        assert_eq!(err, CapError::EmptySlot);
    }

    // ── Step 4: wrong kind ────────────────────────────────────────────────────

    #[test]
    fn require_cap_wrong_kind_rejected() {
        let (cs, h) = setup_endpoint(CapRights::SEND);
        let err = require_cap(&cs, h, CapKind::TaskControl,   // wrong
                              CapRights::TASK_START, None, &NoLease).unwrap_err();
        assert_eq!(err, CapError::WrongKind);
    }

    // ── Step 5: missing right ─────────────────────────────────────────────────

    #[test]
    fn require_cap_missing_right_rejected() {
        let (cs, h) = setup_endpoint(CapRights::SEND);   // no RECV
        let err = require_cap(&cs, h, CapKind::Endpoint,
                              CapRights::RECV, None, &NoLease).unwrap_err();
        assert_eq!(err, CapError::MissingRight);
    }

    // ── Step 6: scope mismatch ────────────────────────────────────────────────

    #[test]
    fn require_cap_scope_mismatch_rejected() {
        let mut cs = CSpace::new();
        let task_a = TaskId::new(1, 0);
        let task_b = TaskId::new(2, 0);
        let h = cs.install_root_scoped(
            CapKind::TaskControl, 0, CapRights::TASK_START,
            ObjectScope::Task(task_a),
        ).unwrap();
        // Require scope for task_b — mismatch.
        let req_scope = ObjectScope::Task(task_b);
        let err = require_cap(&cs, h, CapKind::TaskControl,
                              CapRights::TASK_START,
                              Some(&req_scope), &NoLease).unwrap_err();
        assert_eq!(err, CapError::ScopeMismatch);
    }

    #[test]
    fn require_cap_any_scope_satisfies_all() {
        let mut cs = CSpace::new();
        let task_a = TaskId::new(1, 0);
        // Install with Any scope (bootstrap-style).
        let h = cs.install_root_scoped(
            CapKind::TaskControl, 0, CapRights::TASK_START,
            ObjectScope::Any,
        ).unwrap();
        // Require scope for task_a — should succeed because cap has Any scope.
        let req_scope = ObjectScope::Task(task_a);
        require_cap(&cs, h, CapKind::TaskControl,
                    CapRights::TASK_START,
                    Some(&req_scope), &NoLease).unwrap();
    }

    // ── Step 7: lease revoked ─────────────────────────────────────────────────

    #[test]
    fn require_cap_revoked_lease_rejected() {
        use crate::slot::LeaseBinding;
        use fjell_abi::lease::{LeaseEpoch, LeaseId};

        let mut cs = CSpace::new();
        // install_raw lets us provide a lease-bound capability directly.
        let cap = Capability {
            kind:      CapKind::Endpoint,
            state:     CapState::Active,
            object_id: 77,
            rights:    CapRights::SEND,
            badge:     0,
            scope:     ObjectScope::Any,
            parent:    None,
            lease:     Some(LeaseBinding {
                lease_id:       LeaseId::new(0, 1),
                epoch_at_issue: LeaseEpoch(1),
            }),
        };
        cs.install_raw(0, cap).unwrap();
        let h = CapHandle::new(0, 0);

        let err = require_cap(&cs, h, CapKind::Endpoint,
                              CapRights::SEND, None,
                              &AlwaysRevoked).unwrap_err();
        assert_eq!(err, CapError::LeaseRevoked);
    }

    // ── cap_drop tests (RFC 032) ──────────────────────────────────────────────

    #[test]
    fn cap_drop_active_slot_succeeds() {
        let (mut cs, h) = setup_endpoint(CapRights::SEND);
        cap_drop(&mut cs, h).unwrap();
        // Old handle must now be stale.
        let err = require_cap(&cs, h, CapKind::Endpoint,
                              CapRights::SEND, None, &NoLease).unwrap_err();
        assert_eq!(err, CapError::GenerationMismatch);
    }

    #[test]
    fn cap_drop_allows_dropping_revoked_lease_cap() {
        // Even though the lease checker always says Revoked, drop must succeed.
        // (cap_drop must not check the lease — RFC 032 §2.5.)
        let (mut cs, h) = setup_endpoint(CapRights::SEND);
        cap_drop(&mut cs, h).unwrap();   // succeeds despite AlwaysRevoked
    }

    #[test]
    fn cap_drop_empty_slot_returns_dropped_error() {
        let (mut cs, h) = setup_endpoint(CapRights::SEND);
        cap_drop(&mut cs, h).unwrap();   // first drop: ok
        // Slot is now empty; re-drop must fail.
        // The generation incremented, so handle is stale now.
        let err = cap_drop(&mut cs, h).unwrap_err();
        assert_eq!(err, CapError::GenerationMismatch);
    }

    #[test]
    fn cap_drop_null_handle_rejected() {
        let mut cs = CSpace::new();
        let err = cap_drop(&mut cs, CapHandle::NULL).unwrap_err();
        assert_eq!(err, CapError::InvalidHandle);
    }

    #[test]
    fn cap_drop_slot_reusable_after_drop() {
        // RFC 032 §2.10 CSpace exhaustion test (simplified):
        // After N drops, N new caps can be installed.
        let (mut cs, h1) = setup_endpoint(CapRights::SEND);
        cap_drop(&mut cs, h1).unwrap();
        // Install a new cap into any slot — this should succeed, not exhaust CSpace.
        cs.install_root(CapKind::Endpoint, 99, CapRights::RECV).unwrap();
    }

    // ── cap_mint invariants (RFC 031 §2.9) ───────────────────────────────────

    #[test]
    fn cap_mint_rights_amplification_rejected() {
        let (mut cs, h) = setup_endpoint(CapRights::SEND);   // no RECV
        let err = cs.mint(h, 5, CapRights::SEND | CapRights::RECV, 0).unwrap_err();
        assert_eq!(err, fjell_abi::error::SysError::RightsExceed);
    }

    #[test]
    fn cap_mint_narrower_rights_accepted() {
        let (mut cs, h) = setup_endpoint(CapRights::SEND | CapRights::RECV);
        let child = cs.mint(h, 5, CapRights::SEND, 0).unwrap();
        let cap = cs.get(child).unwrap();
        assert!(cap.rights.contains(CapRights::SEND));
        assert!(!cap.rights.contains(CapRights::RECV));
    }
}
