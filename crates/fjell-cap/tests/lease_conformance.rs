//! Verus conformance: lease epoch revocation.
//! RFC-v0.17-003 §6. Bridges the Verus proof
//! (verification/verus/lease/lease_epoch.rs) to the pure host-testable
//! helpers in `fjell_abi::lease` (`lease_usable`, `lease_revoke_epoch`),
//! which the kernel lease table mirrors.
//!
//! Marker on success: CONFORMANCE:LEASE-EPOCH:PASS

use fjell_abi::lease::{lease_usable, lease_revoke_epoch};

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
    assert!(!lease_usable(false, 7, 7), "revoked/inactive lease must reject");
}

#[test]
fn old_binding_rejected_after_revoke() {
    // LEASE-VERUS-003: a binding usable before revoke is not usable after.
    let epoch_at_issue = 5u32;
    let current = 5u32;
    assert!(lease_usable(true, current, epoch_at_issue), "usable before revoke");

    // revoke: epoch advances, lease becomes inactive.
    let new_epoch = lease_revoke_epoch(current);
    assert_eq!(new_epoch, 6, "LEASE-VERUS-002: revoke increments epoch");
    assert!(!lease_usable(false, new_epoch, epoch_at_issue),
        "stale binding must be unusable after revoke");
}

#[test]
fn revoke_advances_epoch_monotonic() {
    // Mirrors the Verus revoke_advances_epoch lemma over a range.
    for e in [0u32, 1, 41, 1000, u32::MAX - 1] {
        assert_eq!(lease_revoke_epoch(e), e.wrapping_add(1));
        if e != u32::MAX {
            assert!(lease_revoke_epoch(e) > e, "epoch strictly advances (no wrap)");
        }
    }
}

#[test]
fn binding_at_new_epoch_still_inactive() {
    // revoke_blocks_even_new_epoch_binding: even a binding matching the NEW
    // epoch is unusable while the lease is inactive — re-issue needs a fresh
    // active lease.
    let current = 9u32;
    let new_epoch = lease_revoke_epoch(current);
    assert!(!lease_usable(false, new_epoch, new_epoch),
        "inactive lease rejects even an epoch-matching binding");
    println!("CONFORMANCE:LEASE-EPOCH:PASS");
}
