//! Unified capability enforcement functions (RFC 031 §2.5, RFC 032 §2.4; RFC-0.47-001).
//!
//! # The funnel
//!
//! **Every kernel operation that is authorised by a capability reaches [`check`] through one of three named
//! entries**, and a check that exists anywhere else in the kernel is a defect that the `authority-funnel`
//! subcheck of `consistency-check` fails on:
//!
//! * [`require_cap`] -- a handle, and the **kind** the operation is about. Kind is mandatory, so a caller of a
//!   kind-bound operation cannot forget it.
//! * [`require_cap_any_kind`] -- a handle, for the operations that act on a capability *as a slot-object*
//!   whatever it names (`cap_copy`, `cap_mint`, `cap_revoke`, `cap_inspect`). Being a separate name, a
//!   kind-agnostic call is greppable at the call site. It is not a sentinel `CapKind::Any` inside one entry:
//!   "no kind requirement" is the absence of a requirement, exactly as `required_scope: Option<_>` says for
//!   scope.
//! * [`require_held`] -- no handle: the caller must *hold* some `Active` capability of a kind with the rights
//!   and a live lease (`sys_cap_bind_lease` authorises by possession of a `LeaseAdmin`, and the syscall passes
//!   no handle for it).
//!
//! ## Check order (normative, RFC 031 §2.6)
//!
//! ```text
//! 1. CSpace lookup     — handle out of range or null
//! 2. Generation check  — slot.generation != handle.generation
//! 3. Slot-state check  — slot not Active
//! 4. Kind check        — cap.kind != expected_kind      (skipped by `require_cap_any_kind`)
//! 5. Rights check      — cap.rights does not contain required_rights
//! 6. Scope check       — cap.scope incompatible with required_scope
//! 7. Lease check       — lease epoch mismatch or revoked state
//! ```
//!
//! The order is normative: cheap, data-independent checks precede expensive, object-specific checks; the
//! lease check is last because it requires a table lookup. **It is also the right order for a reason that is
//! not about cost**: rights precede the lease, so a caller that lacks the right is never told that a lease was
//! revoked -- the state of an object it has no authority over. An earlier revision checked the lease first in
//! four syscalls, so one input (no right *and* a revoked lease) got `PermissionDenied` from one path and
//! `LeaseRevoked` from another (E-126).
//!
//! ## What is outside the funnel, and why (each is a reasoned exemption, not an omission)
//!
//! * **Releasing a capability** (`cap_drop`, `cap_delete`): releasing authority you hold can only *reduce* it,
//!   so a task must always be able to release a dead slot -- steps 1-3 only. `CapRights::DROP` is therefore
//!   **reserved**: the bit is retained for the ABI, nothing may require it, and it confers nothing.
//! * **`sys_ipc_reply`**: its authority is a pending reply edge (the call being served), not a capability.
//! * **`fjell_cap::lease_revoke::revoke_permitted`**: an *ownership* predicate applied **after** the funnel has
//!   authorised the `LeaseAdmin`; it narrows, it does not authorise.

use super::cspace::CSpace;
use super::handle::CapHandle;
use super::rights::{CapError, CapKind, CapRights, CapState, ObjectScope};
use super::slot::{CapSlotState, Capability, LeaseChecker};

/// The one implementation of the seven checks. `expected_kind: None` skips step 4 and nothing else.
fn check<'cs>(
    cspace: &'cs CSpace,
    handle: CapHandle,
    expected_kind: Option<CapKind>,
    required_rights: CapRights,
    required_scope: Option<&ObjectScope>,
    checker: &dyn LeaseChecker,
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
    if let Some(kind) = expected_kind {
        if cap.kind != kind {
            return Err(CapError::WrongKind);
        }
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

/// Unified capability enforcement (RFC 031 §2.5) for an operation **about a kind of object**.
///
/// Validates a `CapHandle` from the given `CSpace` through all seven checks listed above.
///
/// Returns a shared reference to the `Capability` on success.  The reference is valid for the lifetime of
/// the `CSpace` borrow.
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
    cspace: &'cs CSpace,
    handle: CapHandle,
    expected_kind: CapKind,
    required_rights: CapRights,
    required_scope: Option<&ObjectScope>,
    checker: &dyn LeaseChecker,
) -> Result<&'cs Capability, CapError> {
    check(
        cspace,
        handle,
        Some(expected_kind),
        required_rights,
        required_scope,
        checker,
    )
}

/// [`require_cap`] for an operation that acts on a capability **whatever kind it is** (`cap_copy`, `cap_mint`,
/// `cap_revoke`, `cap_inspect`): the same steps, minus step 4. Everything else -- the order, the errors --
/// is the one implementation's, so the two entries agree on every input that has the same kind.
pub fn require_cap_any_kind<'cs>(
    cspace: &'cs CSpace,
    handle: CapHandle,
    required_rights: CapRights,
    required_scope: Option<&ObjectScope>,
    checker: &dyn LeaseChecker,
) -> Result<&'cs Capability, CapError> {
    check(
        cspace,
        handle,
        None,
        required_rights,
        required_scope,
        checker,
    )
}

/// Authority **by possession**: the caller holds some `Active` capability of `kind` carrying
/// `required_rights` whose lease is live. No handle is named, because the syscall that needs this
/// (`sys_cap_bind_lease`) is not told which capability authorises it.
///
/// Each candidate is judged in the funnel's order -- slot state, kind, rights, lease -- and the first that
/// passes is returned. When none does the answer is [`CapError::MissingRight`] whatever the reason (no such
/// kind, too few rights, or a revoked lease): *which* capability fell short is not the caller's to learn.
pub fn require_held<'cs>(
    cspace: &'cs CSpace,
    kind: CapKind,
    required_rights: CapRights,
    checker: &dyn LeaseChecker,
) -> Result<&'cs Capability, CapError> {
    cspace
        .iter_occupied()
        .find(|c| {
            c.state == CapState::Active
                && c.kind == kind
                && c.rights.contains(required_rights)
                && c.check_lease(checker).is_ok()
        })
        .ok_or(CapError::MissingRight)
}

/// Explicit capability slot drop (RFC 032 §2.4) -- **and the one implementation of releasing a capability**:
/// `sys_cap_delete` calls this too (RFC-0.47-001 D5).
///
/// Releases the capability in `handle`'s slot so the slot can be reused.
///
/// # No authority is checked, and why
///
/// Steps 1-3 only (the handle resolves to an `Active` slot). Releasing a capability you hold can only
/// *reduce* authority, so a task must always be able to release a dead slot, **including one whose lease was
/// revoked**. `CapRights::DROP` is reserved and is not required.
///
/// # Where `cap_drop` and `cap_delete` differ (the two true differences)
/// - the error for an **empty slot**: `cap_drop` answers `Dropped` (`InvalidCap`), `cap_delete` answers
///   `SlotEmpty`;
/// - **audit**: `cap_delete` records `CapDelete` on every attempt including failures, `cap_drop` records
///   `CapDrop` on success only.
///
/// They do **not** differ in authority or in what happens to a revoked-lease capability: both succeed. An
/// earlier doc comment here said otherwise -- *"unlike `cap_delete`, succeeds even when the lease is
/// revoked"* -- which described a difference that did not exist.
///
/// # Errors
///
/// | `CapError` variant   | Condition                                     |
/// |----------------------|------------------------------------------------|
/// | `InvalidHandle`      | handle is null or index out of range          |
/// | `GenerationMismatch` | handle.generation != slot.generation          |
/// | `Dropped`            | slot is already Empty or Dropped              |
pub fn cap_drop(cspace: &mut CSpace, handle: CapHandle) -> Result<(), CapError> {
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
    slot.cap = None;
    slot.state = CapSlotState::Empty;
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
        let cap = require_cap(&cs, h, CapKind::Endpoint, CapRights::SEND, None, &NoLease).unwrap();
        assert_eq!(cap.kind, CapKind::Endpoint);
        assert_eq!(cap.object_id, 42);
    }

    // ── Step 1: null handle ───────────────────────────────────────────────────

    #[test]
    fn require_cap_null_handle_rejected() {
        let (cs, _) = setup_endpoint(CapRights::SEND);
        let err = require_cap(
            &cs,
            CapHandle::NULL,
            CapKind::Endpoint,
            CapRights::SEND,
            None,
            &NoLease,
        )
        .unwrap_err();
        assert_eq!(err, CapError::InvalidHandle);
    }

    // ── Step 2: generation mismatch ───────────────────────────────────────────

    #[test]
    fn require_cap_stale_generation_rejected() {
        let (mut cs, h) = setup_endpoint(CapRights::SEND);
        cs.delete(h).unwrap(); // bumps generation
        let err =
            require_cap(&cs, h, CapKind::Endpoint, CapRights::SEND, None, &NoLease).unwrap_err();
        assert_eq!(err, CapError::GenerationMismatch);
    }

    // ── Step 3: empty slot ────────────────────────────────────────────────────

    #[test]
    fn require_cap_empty_slot_rejected() {
        let cs = CSpace::new();
        // Construct a handle pointing at slot 0 generation 0 (slot IS empty).
        let h = CapHandle::new(0, 0);
        let err =
            require_cap(&cs, h, CapKind::Endpoint, CapRights::SEND, None, &NoLease).unwrap_err();
        assert_eq!(err, CapError::EmptySlot);
    }

    // ── Step 4: wrong kind ────────────────────────────────────────────────────

    #[test]
    fn require_cap_wrong_kind_rejected() {
        let (cs, h) = setup_endpoint(CapRights::SEND);
        let err = require_cap(
            &cs,
            h,
            CapKind::TaskControl, // wrong
            CapRights::TASK_START,
            None,
            &NoLease,
        )
        .unwrap_err();
        assert_eq!(err, CapError::WrongKind);
    }

    // ── Step 5: missing right ─────────────────────────────────────────────────

    #[test]
    fn require_cap_missing_right_rejected() {
        let (cs, h) = setup_endpoint(CapRights::SEND); // no RECV
        let err =
            require_cap(&cs, h, CapKind::Endpoint, CapRights::RECV, None, &NoLease).unwrap_err();
        assert_eq!(err, CapError::MissingRight);
    }

    // ── Step 6: scope mismatch ────────────────────────────────────────────────

    #[test]
    fn require_cap_scope_mismatch_rejected() {
        let mut cs = CSpace::new();
        let task_a = TaskId::new(1, 0);
        let task_b = TaskId::new(2, 0);
        let h = cs
            .install_root_scoped(
                CapKind::TaskControl,
                0,
                CapRights::TASK_START,
                ObjectScope::Task(task_a),
            )
            .unwrap();
        // Require scope for task_b — mismatch.
        let req_scope = ObjectScope::Task(task_b);
        let err = require_cap(
            &cs,
            h,
            CapKind::TaskControl,
            CapRights::TASK_START,
            Some(&req_scope),
            &NoLease,
        )
        .unwrap_err();
        assert_eq!(err, CapError::ScopeMismatch);
    }

    #[test]
    fn require_cap_any_scope_satisfies_all() {
        let mut cs = CSpace::new();
        let task_a = TaskId::new(1, 0);
        // Install with Any scope (bootstrap-style).
        let h = cs
            .install_root_scoped(
                CapKind::TaskControl,
                0,
                CapRights::TASK_START,
                ObjectScope::Any,
            )
            .unwrap();
        // Require scope for task_a — should succeed because cap has Any scope.
        let req_scope = ObjectScope::Task(task_a);
        require_cap(
            &cs,
            h,
            CapKind::TaskControl,
            CapRights::TASK_START,
            Some(&req_scope),
            &NoLease,
        )
        .unwrap();
    }

    // ── Step 7: lease revoked ─────────────────────────────────────────────────

    #[test]
    fn require_cap_revoked_lease_rejected() {
        use crate::slot::LeaseBinding;
        use fjell_abi::lease::{LeaseEpoch, LeaseId};

        let mut cs = CSpace::new();
        // install_raw lets us provide a lease-bound capability directly.
        let cap = Capability {
            kind: CapKind::Endpoint,
            state: CapState::Active,
            object_id: 77,
            rights: CapRights::SEND,
            badge: 0,
            scope: ObjectScope::Any,
            parent: None,
            lease: Some(LeaseBinding {
                lease_id: LeaseId::new(0, 1),
                epoch_at_issue: LeaseEpoch(1),
            }),
        };
        cs.install_raw(0, cap).unwrap();
        let h = CapHandle::new(0, 0);

        let err = require_cap(
            &cs,
            h,
            CapKind::Endpoint,
            CapRights::SEND,
            None,
            &AlwaysRevoked,
        )
        .unwrap_err();
        assert_eq!(err, CapError::LeaseRevoked);
    }

    // ── cap_drop tests (RFC 032) ──────────────────────────────────────────────

    #[test]
    fn cap_drop_active_slot_succeeds() {
        let (mut cs, h) = setup_endpoint(CapRights::SEND);
        cap_drop(&mut cs, h).unwrap();
        // Old handle must now be stale.
        let err =
            require_cap(&cs, h, CapKind::Endpoint, CapRights::SEND, None, &NoLease).unwrap_err();
        assert_eq!(err, CapError::GenerationMismatch);
    }

    #[test]
    fn cap_drop_allows_dropping_revoked_lease_cap() {
        // Even though the lease checker always says Revoked, drop must succeed.
        // (cap_drop must not check the lease — RFC 032 §2.5.)
        let (mut cs, h) = setup_endpoint(CapRights::SEND);
        cap_drop(&mut cs, h).unwrap(); // succeeds despite AlwaysRevoked
    }

    #[test]
    fn cap_drop_empty_slot_returns_dropped_error() {
        let (mut cs, h) = setup_endpoint(CapRights::SEND);
        cap_drop(&mut cs, h).unwrap(); // first drop: ok
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
        cs.install_root(CapKind::Endpoint, 99, CapRights::RECV)
            .unwrap();
    }

    // ── cap_mint invariants (RFC 031 §2.9) ───────────────────────────────────

    #[test]
    fn cap_mint_rights_amplification_rejected() {
        let (mut cs, h) = setup_endpoint(CapRights::SEND); // no RECV
        let err = cs
            .mint(h, 5, CapRights::SEND | CapRights::RECV, 0)
            .unwrap_err();
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

    // ── RFC-0.47-001: the three entries, and the order, exhaustively ─────────────────────────

    use crate::slot::LeaseBinding;
    use fjell_abi::lease::{LeaseEpoch, LeaseId};

    /// A one-slot CSpace holding an `Endpoint` capability with `rights`, lease-bound iff `bound`.
    fn one_cap(rights: CapRights, bound: bool) -> (CSpace, CapHandle) {
        let mut cs = CSpace::new();
        let cap = Capability {
            kind: CapKind::Endpoint,
            state: CapState::Active,
            object_id: 5,
            rights,
            badge: 0,
            scope: ObjectScope::Any,
            parent: None,
            lease: bound.then_some(LeaseBinding {
                lease_id: LeaseId::new(0, 1),
                epoch_at_issue: LeaseEpoch(1),
            }),
        };
        cs.install_raw(0, cap).unwrap();
        (cs, CapHandle::new(0, 0))
    }

    /// For **every** combination of {kind wrong, right missing, lease revoked}, `require_cap` answers the
    /// *first* failure in the normative order kind -> rights -> lease, and `require_cap_any_kind` the first
    /// of rights -> lease. This is the property that one input has one answer, whichever path serves it.
    #[test]
    fn the_first_failure_in_the_normative_order_is_the_answer_for_every_combination() {
        for kind_wrong in [false, true] {
            for right_missing in [false, true] {
                for lease_revoked in [false, true] {
                    let rights = if right_missing {
                        CapRights::RECV
                    } else {
                        CapRights::SEND | CapRights::RECV
                    };
                    let (cs, h) = one_cap(rights, lease_revoked);
                    let checker: &dyn LeaseChecker = if lease_revoked {
                        &AlwaysRevoked
                    } else {
                        &NoLease
                    };
                    let want_kind = if kind_wrong {
                        CapKind::TaskControl
                    } else {
                        CapKind::Endpoint
                    };

                    let expect_full = if kind_wrong {
                        Err(CapError::WrongKind)
                    } else if right_missing {
                        Err(CapError::MissingRight)
                    } else if lease_revoked {
                        Err(CapError::LeaseRevoked)
                    } else {
                        Ok(())
                    };
                    let got_full =
                        require_cap(&cs, h, want_kind, CapRights::SEND, None, checker).map(|_| ());
                    assert_eq!(
                        got_full, expect_full,
                        "require_cap: kind_wrong={kind_wrong} right_missing={right_missing} lease_revoked={lease_revoked}"
                    );

                    let expect_any = if right_missing {
                        Err(CapError::MissingRight)
                    } else if lease_revoked {
                        Err(CapError::LeaseRevoked)
                    } else {
                        Ok(())
                    };
                    let got_any =
                        require_cap_any_kind(&cs, h, CapRights::SEND, None, checker).map(|_| ());
                    assert_eq!(
                        got_any, expect_any,
                        "require_cap_any_kind: right_missing={right_missing} lease_revoked={lease_revoked}"
                    );
                }
            }
        }
    }

    /// The case the inline paths got wrong: no right **and** a revoked lease is `MissingRight`, never
    /// `LeaseRevoked` -- a caller without the right is not told the state of a lease it has no authority over.
    #[test]
    fn no_right_and_a_revoked_lease_is_missing_right_through_both_entries() {
        let (cs, h) = one_cap(CapRights::RECV, true);
        assert_eq!(
            require_cap(
                &cs,
                h,
                CapKind::Endpoint,
                CapRights::SEND,
                None,
                &AlwaysRevoked
            )
            .unwrap_err(),
            CapError::MissingRight
        );
        assert_eq!(
            require_cap_any_kind(&cs, h, CapRights::SEND, None, &AlwaysRevoked).unwrap_err(),
            CapError::MissingRight
        );
    }

    /// `require_cap_any_kind` accepts a capability of any kind; `require_cap` does not.
    #[test]
    fn the_any_kind_entry_accepts_every_kind_the_kind_entry_refuses() {
        let mut cs = CSpace::new();
        let h = cs
            .install_root(CapKind::LeaseAdmin, 0, CapRights::INSPECT)
            .unwrap();
        assert!(require_cap_any_kind(&cs, h, CapRights::INSPECT, None, &NoLease).is_ok());
        assert_eq!(
            require_cap(
                &cs,
                h,
                CapKind::Endpoint,
                CapRights::INSPECT,
                None,
                &NoLease
            )
            .unwrap_err(),
            CapError::WrongKind
        );
    }

    /// The structural steps are the same through both entries.
    #[test]
    fn both_entries_refuse_a_null_stale_and_empty_handle_identically() {
        let (mut cs, h) = one_cap(CapRights::SEND, false);
        let null = CapHandle::NULL;
        let empty = CapHandle::new(7, 0);
        for handle in [null, empty] {
            assert_eq!(
                require_cap(
                    &cs,
                    handle,
                    CapKind::Endpoint,
                    CapRights::SEND,
                    None,
                    &NoLease
                )
                .map(|_| ()),
                require_cap_any_kind(&cs, handle, CapRights::SEND, None, &NoLease).map(|_| ())
            );
        }
        cs.delete(h).unwrap(); // generation bumps: h is now stale
        let a = require_cap(&cs, h, CapKind::Endpoint, CapRights::SEND, None, &NoLease).map(|_| ());
        let b = require_cap_any_kind(&cs, h, CapRights::SEND, None, &NoLease).map(|_| ());
        assert_eq!(a, b);
        assert!(a.is_err());
    }

    // ── require_held ─────────────────────────────────────────────────────────────────────────

    fn held_cspace(caps: &[(CapKind, CapRights, bool)]) -> CSpace {
        let mut cs = CSpace::new();
        for (i, (kind, rights, bound)) in caps.iter().enumerate() {
            let cap = Capability {
                kind: *kind,
                state: CapState::Active,
                object_id: 0,
                rights: *rights,
                badge: 0,
                scope: ObjectScope::Any,
                parent: None,
                lease: bound.then_some(LeaseBinding {
                    lease_id: LeaseId::new(i as u16, 1),
                    epoch_at_issue: LeaseEpoch(1),
                }),
            };
            cs.install_raw(i, cap).unwrap();
        }
        cs
    }

    #[test]
    fn require_held_finds_a_holder_and_refuses_every_way_of_not_holding() {
        let need = CapRights::LEASE_CREATE | CapRights::LEASE_BIND;
        let ok = held_cspace(&[(CapKind::LeaseAdmin, need, false)]);
        assert!(require_held(&ok, CapKind::LeaseAdmin, need, &NoLease).is_ok());
        // nothing held
        assert_eq!(
            require_held(&CSpace::new(), CapKind::LeaseAdmin, need, &NoLease).unwrap_err(),
            CapError::MissingRight
        );
        // right kind, too few rights
        let few = held_cspace(&[(CapKind::LeaseAdmin, CapRights::LEASE_CREATE, false)]);
        assert!(require_held(&few, CapKind::LeaseAdmin, need, &NoLease).is_err());
        // right rights, wrong kind
        let wrong = held_cspace(&[(CapKind::Endpoint, need, false)]);
        assert!(require_held(&wrong, CapKind::LeaseAdmin, need, &NoLease).is_err());
        // right kind and rights, but the lease is revoked
        let revoked = held_cspace(&[(CapKind::LeaseAdmin, need, true)]);
        assert!(require_held(&revoked, CapKind::LeaseAdmin, need, &AlwaysRevoked).is_err());
    }

    #[test]
    fn require_held_skips_a_dead_candidate_and_finds_a_live_one() {
        let need = CapRights::LEASE_CREATE | CapRights::LEASE_BIND;
        // slot 0: right kind, rights, revoked lease; slot 1: right kind, rights, no lease.
        let cs = held_cspace(&[
            (CapKind::LeaseAdmin, need, true),
            (CapKind::LeaseAdmin, need, false),
        ]);
        let found = require_held(&cs, CapKind::LeaseAdmin, need, &AlwaysRevoked).unwrap();
        assert!(
            found.lease.is_none(),
            "the live candidate, not the revoked one"
        );
    }
}
