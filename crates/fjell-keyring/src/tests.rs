//! Host unit tests for `fjell-keyring`.
//!
//! Source of truth: RFC-v0.3-002 §11.1.
//!
//! Each test maps to a bullet in the RFC's "Host unit tests" enumeration.

extern crate alloc;

use crate::algorithm::SignatureAlgorithm;
use crate::anchor::{AuthorityClass, TrustAnchor};
use crate::dev_provider::DevSignatureProvider;
use crate::epoch::KeyEpoch;
use crate::error::SigError;
use crate::keyring::{Keyring, PURPOSE_SLOT_COUNT};
use crate::provider::SignatureProvider;
use crate::snapshot::{KeyringSnapshot, MAX_SNAPSHOT_ANCHORS};
use crate::ANCHORS_PER_PURPOSE;
use crate::SCHEMA_VERSION;
use fjell_trust_provider::KeyPurpose;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn anchor_dev(purpose: KeyPurpose, epoch: u32, byte: u8) -> TrustAnchor {
    let key = [byte; 32];
    TrustAnchor::new(
        purpose,
        SignatureAlgorithm::DevDigest32,
        AuthorityClass::Standard,
        KeyEpoch(epoch),
        &key,
    )
    .expect("anchor fits")
}

fn anchor_ed(purpose: KeyPurpose, epoch: u32, byte: u8) -> TrustAnchor {
    let key = [byte; 32];
    TrustAnchor::new(
        purpose,
        SignatureAlgorithm::Ed25519,
        AuthorityClass::Genesis,
        KeyEpoch(epoch),
        &key,
    )
    .expect("anchor fits")
}

// ===========================================================================
// A — Algorithm / Anchor / Epoch type contracts
// ===========================================================================

#[test]
fn algorithm_tags_are_stable() {
    assert_eq!(SignatureAlgorithm::Ed25519.tag(), 0x01);
    assert_eq!(SignatureAlgorithm::EcdsaP256.tag(), 0x02);
    assert_eq!(SignatureAlgorithm::DevDigest32.tag(), 0xFE);
}

#[test]
fn algorithm_release_permitted_classification() {
    assert!(SignatureAlgorithm::Ed25519.permitted_in_release());
    assert!(SignatureAlgorithm::EcdsaP256.permitted_in_release());
    assert!(!SignatureAlgorithm::DevDigest32.permitted_in_release());
}

#[test]
fn algorithm_signature_lengths_documented() {
    assert_eq!(SignatureAlgorithm::Ed25519.signature_len(), 64);
    assert_eq!(SignatureAlgorithm::EcdsaP256.signature_len(), 64);
    assert_eq!(SignatureAlgorithm::DevDigest32.signature_len(), 32);
}

#[test]
fn authority_class_tags_are_stable() {
    assert_eq!(AuthorityClass::Genesis.tag(), 0x01);
    assert_eq!(AuthorityClass::Standard.tag(), 0x02);
}

#[test]
fn anchor_rejects_oversize_key() {
    let too_big = [0u8; 128]; // > ANCHOR_KEY_BYTES_MAX (64)
    let a = TrustAnchor::new(
        KeyPurpose::ReleaseVerification,
        SignatureAlgorithm::Ed25519,
        AuthorityClass::Genesis,
        KeyEpoch::ONE,
        &too_big,
    );
    assert!(a.is_none());
}

#[test]
fn anchor_preserves_key_bytes() {
    let key = [0xAB; 16];
    let a = TrustAnchor::new(
        KeyPurpose::PolicyVerification,
        SignatureAlgorithm::Ed25519,
        AuthorityClass::Genesis,
        KeyEpoch(7),
        &key,
    )
    .unwrap();
    assert_eq!(a.key_len, 16);
    assert_eq!(a.key(), &key);
}

#[test]
fn epoch_zero_is_sentinel() {
    assert_eq!(KeyEpoch::ZERO.raw(), 0);
    assert_eq!(KeyEpoch::ONE.raw(), 1);
    assert!(KeyEpoch::ONE > KeyEpoch::ZERO);
}

// ===========================================================================
// B — Keyring container: install / lookup / epoch
// ===========================================================================

#[test]
fn keyring_starts_empty_and_not_in_release_mode() {
    let k = Keyring::new();
    assert!(k.is_empty());
    assert_eq!(k.len(), 0);
    assert!(!k.release_mode());
}

#[test]
fn keyring_install_records_anchor() {
    let mut k = Keyring::new();
    k.install(anchor_dev(KeyPurpose::ReleaseVerification, 1, 0xA0))
        .expect("install");
    assert_eq!(k.len(), 1);
    let a = k.latest(KeyPurpose::ReleaseVerification).unwrap();
    assert_eq!(a.epoch.raw(), 1);
    assert_eq!(k.active_epoch(KeyPurpose::ReleaseVerification).unwrap(), KeyEpoch(1));
}

#[test]
fn keyring_install_advances_active_epoch() {
    let mut k = Keyring::new();
    k.install(anchor_dev(KeyPurpose::ReleaseVerification, 1, 0x10))
        .unwrap();
    k.install(anchor_dev(KeyPurpose::ReleaseVerification, 2, 0x20))
        .unwrap();
    k.install(anchor_dev(KeyPurpose::ReleaseVerification, 5, 0x50))
        .unwrap();
    assert_eq!(
        k.active_epoch(KeyPurpose::ReleaseVerification).unwrap(),
        KeyEpoch(5)
    );
    assert_eq!(
        k.latest(KeyPurpose::ReleaseVerification).unwrap().epoch.raw(),
        5
    );
}

#[test]
fn keyring_rejects_epoch_regression() {
    let mut k = Keyring::new();
    k.install(anchor_dev(KeyPurpose::ReleaseVerification, 3, 0x30))
        .unwrap();
    let err = k
        .install(anchor_dev(KeyPurpose::ReleaseVerification, 2, 0x20))
        .unwrap_err();
    assert_eq!(err, SigError::EpochRegression);
    // Equal epoch must also be rejected.
    let err = k
        .install(anchor_dev(KeyPurpose::ReleaseVerification, 3, 0x33))
        .unwrap_err();
    assert_eq!(err, SigError::EpochRegression);
}

#[test]
fn keyring_release_mode_rejects_dev_digest32() {
    let mut k = Keyring::new();
    // Genesis Ed25519 anchor allowed in either mode.
    k.install(anchor_ed(KeyPurpose::AttestationSigning, 1, 0xA1))
        .unwrap();
    k.enter_release_mode();
    assert!(k.release_mode());
    let err = k
        .install(anchor_dev(KeyPurpose::AttestationSigning, 5, 0x55))
        .unwrap_err();
    assert_eq!(err, SigError::ReleaseModeViolation);
}

#[test]
fn keyring_release_mode_is_one_way() {
    let mut k = Keyring::new();
    assert!(!k.release_mode());
    k.enter_release_mode();
    assert!(k.release_mode());
    k.enter_release_mode();
    assert!(k.release_mode());
}

#[test]
fn keyring_purpose_isolation() {
    let mut k = Keyring::new();
    k.install(anchor_dev(KeyPurpose::ReleaseVerification, 1, 0xA0))
        .unwrap();
    k.install(anchor_dev(KeyPurpose::PolicyVerification, 1, 0xB0))
        .unwrap();
    // The lookup must return the right purpose's anchor.
    let r = k.latest(KeyPurpose::ReleaseVerification).unwrap();
    let p = k.latest(KeyPurpose::PolicyVerification).unwrap();
    assert_eq!(r.purpose, KeyPurpose::ReleaseVerification);
    assert_eq!(p.purpose, KeyPurpose::PolicyVerification);
    assert_eq!(r.key()[0], 0xA0);
    assert_eq!(p.key()[0], 0xB0);
}

#[test]
fn keyring_evicts_oldest_when_full() {
    let mut k = Keyring::new();
    // Fill ANCHORS_PER_PURPOSE slots for one purpose.
    for i in 1..=ANCHORS_PER_PURPOSE as u32 {
        k.install(anchor_dev(KeyPurpose::ReleaseVerification, i, i as u8))
            .unwrap();
    }
    // Install one more — should evict the lowest-epoch entry (1).
    let next_epoch = (ANCHORS_PER_PURPOSE as u32) + 5;
    k.install(anchor_dev(
        KeyPurpose::ReleaseVerification,
        next_epoch,
        0xFF,
    ))
    .unwrap();
    // Oldest (epoch 1) should be gone.
    let still_present: alloc::vec::Vec<u32> = k
        .anchors_for(KeyPurpose::ReleaseVerification)
        .map(|a| a.epoch.raw())
        .collect();
    assert!(!still_present.contains(&1));
    assert!(still_present.contains(&next_epoch));
    assert_eq!(still_present.len(), ANCHORS_PER_PURPOSE);
}

#[test]
fn keyring_anchors_for_iterates_only_matching_purpose() {
    let mut k = Keyring::new();
    k.install(anchor_dev(KeyPurpose::ReleaseVerification, 1, 0x01))
        .unwrap();
    k.install(anchor_dev(KeyPurpose::ReleaseVerification, 2, 0x02))
        .unwrap();
    k.install(anchor_dev(KeyPurpose::AttestationSigning, 1, 0x10))
        .unwrap();
    let release_count = k.anchors_for(KeyPurpose::ReleaseVerification).count();
    let attest_count = k.anchors_for(KeyPurpose::AttestationSigning).count();
    let policy_count = k.anchors_for(KeyPurpose::PolicyVerification).count();
    assert_eq!(release_count, 2);
    assert_eq!(attest_count, 1);
    assert_eq!(policy_count, 0);
}

#[test]
fn keyring_purpose_slot_count_matches_keypurpose_all() {
    assert_eq!(PURPOSE_SLOT_COUNT, KeyPurpose::all().len());
}

// ===========================================================================
// C — DevSignatureProvider (development algorithm path)
// ===========================================================================

#[test]
fn dev_provider_supports_only_dev_digest32() {
    let p = DevSignatureProvider::new();
    assert!(p.supports(SignatureAlgorithm::DevDigest32));
    assert!(!p.supports(SignatureAlgorithm::Ed25519));
    assert!(!p.supports(SignatureAlgorithm::EcdsaP256));
}

#[test]
fn dev_provider_sign_then_verify_round_trip() {
    let p = DevSignatureProvider::new();
    let a = anchor_dev(KeyPurpose::AttestationSigning, 1, 0x9A);
    let payload = b"hello fjell";
    let mut out = [0u8; 64];
    let n = p.sign(&a, payload, &mut out).expect("sign");
    assert_eq!(n, 32);
    p.verify(&a, payload, &out[..n]).expect("verify");
}

#[test]
fn dev_provider_verify_rejects_wrong_payload() {
    let p = DevSignatureProvider::new();
    let a = anchor_dev(KeyPurpose::AttestationSigning, 1, 0x9A);
    let mut out = [0u8; 64];
    let n = p.sign(&a, b"payload-a", &mut out).unwrap();
    let err = p.verify(&a, b"payload-b", &out[..n]).unwrap_err();
    assert_eq!(err, SigError::SignatureVerifyFailed);
}

#[test]
fn dev_provider_verify_rejects_wrong_anchor_key() {
    let p = DevSignatureProvider::new();
    let a1 = anchor_dev(KeyPurpose::AttestationSigning, 1, 0xAA);
    let a2 = anchor_dev(KeyPurpose::AttestationSigning, 1, 0xBB);
    let payload = b"verify me";
    let mut out = [0u8; 64];
    let n = p.sign(&a1, payload, &mut out).unwrap();
    let err = p.verify(&a2, payload, &out[..n]).unwrap_err();
    assert_eq!(err, SigError::SignatureVerifyFailed);
}

#[test]
fn dev_provider_verify_rejects_wrong_epoch() {
    let p = DevSignatureProvider::new();
    let a1 = anchor_dev(KeyPurpose::AttestationSigning, 1, 0xCC);
    let a2 = anchor_dev(KeyPurpose::AttestationSigning, 2, 0xCC);
    let payload = b"x";
    let mut out = [0u8; 64];
    let n = p.sign(&a1, payload, &mut out).unwrap();
    let err = p.verify(&a2, payload, &out[..n]).unwrap_err();
    assert_eq!(err, SigError::SignatureVerifyFailed);
}

#[test]
fn dev_provider_rejects_non_dev_algorithm() {
    let p = DevSignatureProvider::new();
    let a = anchor_ed(KeyPurpose::ReleaseVerification, 1, 0x10);
    let payload = b"x";
    let sig = [0u8; 32];
    let err = p.verify(&a, payload, &sig).unwrap_err();
    assert_eq!(err, SigError::SignatureVerifyFailed);
    // sign() must refuse too.
    let mut out = [0u8; 64];
    let err = p.sign(&a, payload, &mut out).unwrap_err();
    assert_eq!(err, SigError::ReleaseModeViolation);
}

#[test]
fn dev_provider_verify_rejects_truncated_signature() {
    let p = DevSignatureProvider::new();
    let a = anchor_dev(KeyPurpose::AttestationSigning, 1, 0xAA);
    let payload = b"x";
    let mut out = [0u8; 64];
    let n = p.sign(&a, payload, &mut out).unwrap();
    // Truncated by one byte.
    let err = p.verify(&a, payload, &out[..n - 1]).unwrap_err();
    assert_eq!(err, SigError::SignatureVerifyFailed);
}

// ===========================================================================
// D — KeyringSnapshot persistence
// ===========================================================================

#[test]
fn snapshot_from_empty_keyring_is_well_formed() {
    let k = Keyring::new();
    let snap = KeyringSnapshot::from_keyring(&k);
    assert_eq!(snap.schema_version, SCHEMA_VERSION);
    assert_eq!(snap.anchor_count, 0);
    let recomputed = {
        // Walk the same path the constructor walks.
        let mut s = KeyringSnapshot {
            schema_version: snap.schema_version,
            anchor_count: snap.anchor_count,
            anchors: snap.anchors,
            snapshot_digest: fjell_measure_format::Digest32::ZERO,
        };
        s.snapshot_digest = fjell_measure_format::Digest32::ZERO;
        // Recompute by re-building from keyring.
        KeyringSnapshot::from_keyring(&k).snapshot_digest
    };
    assert_eq!(snap.snapshot_digest, recomputed);
}

#[test]
fn snapshot_round_trip_preserves_anchors() {
    let mut k1 = Keyring::new();
    k1.install(anchor_dev(KeyPurpose::ReleaseVerification, 1, 0x11))
        .unwrap();
    k1.install(anchor_dev(KeyPurpose::ReleaseVerification, 2, 0x22))
        .unwrap();
    k1.install(anchor_ed(KeyPurpose::AttestationSigning, 1, 0xAA))
        .unwrap();
    k1.install(anchor_ed(KeyPurpose::PolicyVerification, 5, 0xCC))
        .unwrap();

    let snap = KeyringSnapshot::from_keyring(&k1);
    assert!(snap.anchor_count >= 4);

    let mut k2 = Keyring::new();
    let installed = snap.apply_to(&mut k2).expect("apply");
    assert_eq!(installed, snap.anchor_count as usize);

    // active_epoch comparisons survive the round-trip.
    assert_eq!(
        k2.active_epoch(KeyPurpose::ReleaseVerification).unwrap(),
        k1.active_epoch(KeyPurpose::ReleaseVerification).unwrap()
    );
    assert_eq!(
        k2.active_epoch(KeyPurpose::AttestationSigning).unwrap(),
        k1.active_epoch(KeyPurpose::AttestationSigning).unwrap()
    );
    assert_eq!(
        k2.active_epoch(KeyPurpose::PolicyVerification).unwrap(),
        k1.active_epoch(KeyPurpose::PolicyVerification).unwrap()
    );
}

#[test]
fn snapshot_apply_rejects_tampered_digest() {
    let mut k = Keyring::new();
    k.install(anchor_dev(KeyPurpose::AttestationSigning, 1, 0x99))
        .unwrap();
    let mut snap = KeyringSnapshot::from_keyring(&k);
    // Flip a byte in the recorded digest.
    snap.snapshot_digest.0[0] ^= 0xFF;
    let mut k2 = Keyring::new();
    let err = snap.apply_to(&mut k2).unwrap_err();
    assert_eq!(err, SigError::SnapshotDigestMismatch);
}

#[test]
fn snapshot_max_anchors_constant_matches_layout() {
    assert_eq!(MAX_SNAPSHOT_ANCHORS, PURPOSE_SLOT_COUNT * ANCHORS_PER_PURPOSE);
}

// ===========================================================================
// E — SigError code stability
// ===========================================================================

#[test]
fn sig_error_codes_are_stable() {
    assert_eq!(SigError::AlgorithmForbiddenInRelease.code(), 0x0001);
    assert_eq!(SigError::EpochRegression.code(), 0x0002);
    assert_eq!(SigError::NoAnchorForPurpose.code(), 0x0003);
    assert_eq!(SigError::SignatureVerifyFailed.code(), 0x0004);
    assert_eq!(SigError::ReleaseModeViolation.code(), 0x0005);
    assert_eq!(SigError::AnchorsCapacityExhausted.code(), 0x0006);
    assert_eq!(SigError::SnapshotMalformed.code(), 0x0007);
    assert_eq!(SigError::SnapshotDigestMismatch.code(), 0x0008);
    assert_eq!(SigError::Internal.code(), 0xFFFF);
}
