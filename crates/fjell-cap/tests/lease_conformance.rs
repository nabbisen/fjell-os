//! Verus conformance: lease epoch revocation.
//! RFC-v0.17-003 §6. Bridges the Verus proof
//! (verification/verus/lease/lease_epoch.rs) to the pure host-testable
//! helpers in `fjell_abi::lease` (`lease_usable`, `lease_revoke`), which the
//! kernel lease table mirrors. Includes the architect-C6 retire-before-wrap
//! boundary tests (epoch = 0, 1, MAX-1, MAX).
//!
//! Marker on success: CONFORMANCE:LEASE-EPOCH:PASS

use fjell_abi::lease::{
    Alloc, IdRelation, RevokeOutcome, SlotKind, SlotView, lease_alloc, lease_can_recycle,
    lease_id_relation, lease_revoke, lease_usable, lease_usable_in_incarnation,
};

#[test]
fn active_matching_epoch_accepted() {
    // LEASE-VERUS-001
    assert!(lease_usable(true, 7, 7));
}

#[test]
fn active_nonmatching_epoch_rejected() {
    assert!(!lease_usable(true, 8, 7), "epoch mismatch must reject");
}

#[test]
fn inactive_lease_rejected() {
    assert!(
        !lease_usable(false, 7, 7),
        "revoked/inactive lease must reject"
    );
}

#[test]
fn old_binding_rejected_after_revoke() {
    // LEASE-VERUS-003: a binding usable before revoke is not usable after.
    let epoch_at_issue = 5u32;
    let current = 5u32;
    assert!(
        lease_usable(true, current, epoch_at_issue),
        "usable before revoke"
    );

    // revoke: epoch advances, lease becomes inactive.
    let new_epoch = match lease_revoke(current) {
        RevokeOutcome::Advanced(e) => e,
        RevokeOutcome::MustRetire => panic!("epoch 5 is inside the bounded domain"),
    };
    assert_eq!(new_epoch, 6, "LEASE-VERUS-002: revoke increments epoch");
    assert!(
        !lease_usable(false, new_epoch, epoch_at_issue),
        "stale binding must be unusable after revoke"
    );
}

#[test]
fn revoke_advances_epoch_monotonic() {
    // Mirrors the Verus revoke_advances_epoch lemma over the bounded domain
    // (LEASE-VERUS-005: Advanced(old + 1), strictly increasing, never wraps).
    for e in [0u32, 1, 41, 1000, u32::MAX - 1] {
        match lease_revoke(e) {
            RevokeOutcome::Advanced(n) => {
                assert_eq!(n, e + 1, "Advanced carries old + 1");
                assert!(n > e, "epoch strictly advances (no wrap)");
            }
            RevokeOutcome::MustRetire => panic!("{e} is inside the bounded domain"),
        }
    }
}

// ── Architect C6 boundary tests: retire-before-wrap ───────────────────────────

#[test]
fn revoke_boundary_epoch_zero() {
    assert_eq!(lease_revoke(0), RevokeOutcome::Advanced(1));
}

#[test]
fn revoke_boundary_epoch_one() {
    assert_eq!(lease_revoke(1), RevokeOutcome::Advanced(2));
}

#[test]
fn revoke_boundary_epoch_max_minus_one() {
    // The last advance in the domain lands exactly on MAX.
    assert_eq!(
        lease_revoke(u32::MAX - 1),
        RevokeOutcome::Advanced(u32::MAX)
    );
}

#[test]
fn revoke_boundary_epoch_max_must_retire() {
    // At MAX the lease MUST be retired, never wrapped (C6).
    assert_eq!(lease_revoke(u32::MAX), RevokeOutcome::MustRetire);
}

#[test]
fn binding_at_new_epoch_still_inactive() {
    // revoke_blocks_even_new_epoch_binding: even a binding matching the NEW
    // epoch is unusable while the lease is inactive — re-issue needs a fresh
    // active lease.
    let current = 9u32;
    let new_epoch = match lease_revoke(current) {
        RevokeOutcome::Advanced(e) => e,
        RevokeOutcome::MustRetire => panic!("epoch 9 is inside the bounded domain"),
    };
    assert!(
        !lease_usable(false, new_epoch, new_epoch),
        "inactive lease rejects even an epoch-matching binding"
    );
    println!("CONFORMANCE:LEASE-EPOCH:PASS");
}

// ── Slot reuse (RFC-0.46-001): the host bridge to LEASE-VERUS-006..012 ──────────────────────────
//
// The Verus model proves these over `nat`; this drives the u16/u32 mirror in `fjell_abi::lease` over
// the same cases, including the boundaries.

/// LEASE-VERUS-008: revoke, reclaim, and a binding from before is dead -- for any new epoch.
#[test]
fn recycled_slot_is_not_its_predecessor_for_any_new_epoch() {
    let (inc, epoch) = (3u16, 8u32);
    let revoked_epoch = match lease_revoke(epoch) {
        RevokeOutcome::Advanced(e) => e,
        RevokeOutcome::MustRetire => panic!("inside the domain"),
    };
    assert!(lease_can_recycle(SlotKind::Revoked, inc, revoked_epoch));
    // Every plausible new epoch, including the old binding's own (the collision case).
    for new_epoch in [1u32, epoch, revoked_epoch, revoked_epoch + 1, u32::MAX - 1] {
        assert!(
            !lease_usable_in_incarnation(true, inc + 1, new_epoch, inc, epoch),
            "new epoch {new_epoch}"
        );
    }
}

/// LEASE-VERUS-009: generation advances by one and stays in u16; LEASE-VERUS-010: only revoked slots.
#[test]
fn reclamation_is_for_revoked_slots_below_both_ceilings() {
    assert!(lease_can_recycle(SlotKind::Revoked, 0, 2));
    assert!(
        lease_can_recycle(SlotKind::Revoked, u16::MAX - 1, 2),
        "the last reuse in the domain"
    );
    assert!(
        !lease_can_recycle(SlotKind::Revoked, u16::MAX, 2),
        "retired: no u16 wrap"
    );
    assert!(
        !lease_can_recycle(SlotKind::Revoked, 0, u32::MAX),
        "retired: no u32 wrap"
    );
    assert!(
        !lease_can_recycle(SlotKind::Active, 0, 2),
        "a live lease is never reused"
    );
    assert!(!lease_can_recycle(SlotKind::Empty, 0, 0));
}

/// LEASE-VERUS-011: no ABA at distance two (generation g, then g+1, then g+2).
#[test]
fn a_binding_stays_dead_across_two_reuses() {
    let (gen0, epoch0) = (0u16, 4u32);
    for inc in [gen0 + 1, gen0 + 2] {
        for new_epoch in [epoch0, epoch0 + 1, epoch0 + 2] {
            assert!(!lease_usable_in_incarnation(
                true, inc, new_epoch, gen0, epoch0
            ));
        }
    }
}

/// LEASE-VERUS-012: the new incarnation's own binding works.
#[test]
fn a_binding_issued_against_the_new_incarnation_is_usable() {
    assert!(lease_usable_in_incarnation(true, 5, 9, 5, 9));
    assert_eq!(lease_id_relation(5, 5), IdRelation::Current);
}

/// The allocator prefers a never-used slot and otherwise goes round-robin over reclaimable ones.
#[test]
fn allocation_is_fresh_then_round_robin() {
    let v = |kind, generation, epoch| SlotView {
        kind,
        generation,
        epoch,
    };
    let s = [
        v(SlotKind::Revoked, 0, 2),
        v(SlotKind::Revoked, 0, 2),
        v(SlotKind::Revoked, 0, 2),
    ];
    assert_eq!(lease_alloc(&s, 1), Alloc::Recycle(1));
    assert_eq!(lease_alloc(&s, 2), Alloc::Recycle(2));
    let mut s2 = s;
    s2[1] = v(SlotKind::Empty, 0, 0);
    assert_eq!(
        lease_alloc(&s2, 2),
        Alloc::Fresh(1),
        "an Empty slot wins over the cursor"
    );
}
