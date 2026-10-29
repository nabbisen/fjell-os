//! Host unit tests for `AttestationRecordV2` (RFC v0.3-004 §11.1).
//!
//! Covers: digest coverage, mutation sensitivity, signed-by envelope
//! exclusion, round-trip, profile-type gating, and dev/v1 compatibility.

#![allow(unused_imports)] // test fixtures; future tests will use these
extern crate alloc;

use crate::v2::{
    AttestationRecordV2, ClaimAvailability, FreshnessClaimsV2, KeyringClaims, NonceClass,
    ProviderClaims, RollbackClaims, SignedAttestationRecordV2, SignedByDescriptor,
};
use crate::{AttestationProfile, AttestationRecordId, MeasurementClaims, ProvenanceClaims};
use fjell_measure_format::Digest32;
use fjell_trust_provider::{
    development::DevelopmentTrustProvider,
    ids::TrustProviderId,
    profile::{TrustProfile, TrustProviderKind},
};
use fjell_upgrade_format::rollback_record::AdvanceSource;

// ── Fixtures ─────────────────────────────────────────────────────────────────

fn chain_reading() -> MeasurementClaims {
    MeasurementClaims {
        head_seq: 4,
        chain_digest: Digest32([0x5C; 32]),
        included_from_seq: 1,
        included_to_seq: 4,
    }
}

/// What the local factory actually produces: only the measurement is real.
fn truthful_record() -> AttestationRecordV2 {
    AttestationRecordV2::local(
        AttestationRecordId(*b"AT000001"),
        TrustProviderId::new(1),
        *b"stable\0\0",
        [0u8; 16],
        42,
        1,
        Some(chain_reading()),
    )
}

/// A record in which EVERY claim is available and carries a distinct value, so a
/// test can show that each field is covered by the digest. (Not something any
/// service produces today; the factory refuses to invent these.)
fn dev_record() -> AttestationRecordV2 {
    let mut r = truthful_record();
    r.availability = ClaimAvailability {
        kernel_digest: true,
        release_digest: true,
        rootfs_digest: true,
        policy_digest: true,
        snapshot_digest: true,
        measurement: true,
    };
    r.boot.kernel_digest = Digest32([0x11; 32]);
    r.verification.release_digest = Digest32([0x22; 32]);
    r.verification.rootfs_digest = Digest32([0x33; 32]);
    r.verification.policy_digest = Digest32([0x44; 32]);
    r.verification.release_verified = true;
    r.verification.rootfs_verified = true;
    r.verification.policy_verified = true;
    r.snapshot.snapshot_digest = Digest32([0x66; 32]);
    r
}

fn dev_provider() -> DevelopmentTrustProvider {
    DevelopmentTrustProvider::with_default_key(TrustProviderId::new(1), 1)
}

fn dev_signed_by() -> SignedByDescriptor {
    SignedByDescriptor {
        provider_id: TrustProviderId::new(1),
        provider_generation: 1,
        keyring_anchor_epoch: 1,
        algorithm: 0xFE, // DevDigest32 tag
    }
}

// ── Digest-coverage tests ────────────────────────────────────────────────────

#[test]
fn v2_digest_covers_provider_id() {
    let r1 = dev_record();
    let mut r2 = r1;
    r2.provider.provider_id = TrustProviderId::new(99);
    assert_ne!(r1.canonical_digest(), r2.canonical_digest());
}

#[test]
fn v2_digest_covers_keyring_active_epochs() {
    let r1 = dev_record();
    let mut r2 = r1;
    r2.keyring.active_epoch_attestation = 99;
    assert_ne!(r1.canonical_digest(), r2.canonical_digest());
}

#[test]
fn v2_digest_covers_keyring_snapshot_digest() {
    let r1 = dev_record();
    let mut r2 = r1;
    r2.keyring.keyring_snapshot_digest = Digest32([0xFF; 32]);
    assert_ne!(r1.canonical_digest(), r2.canonical_digest());
}

#[test]
fn v2_digest_covers_rollback_min_counter() {
    let r1 = dev_record();
    let mut r2 = r1;
    r2.rollback.min_counter = 999;
    assert_ne!(r1.canonical_digest(), r2.canonical_digest());
}

#[test]
fn v2_digest_covers_rollback_channel_id() {
    let r1 = dev_record();
    let mut r2 = r1;
    r2.rollback.channel_id = *b"lts\0\0\0\0\0";
    assert_ne!(r1.canonical_digest(), r2.canonical_digest());
}

#[test]
fn v2_digest_covers_nonce_bytes() {
    let r1 = dev_record();
    let mut r2 = r1;
    r2.freshness.nonce_bytes = [0xAA; 16];
    assert_ne!(r1.canonical_digest(), r2.canonical_digest());
}

#[test]
fn v2_digest_covers_measurement_chain_digest() {
    let r1 = dev_record();
    let mut r2 = r1;
    r2.measurement.chain_digest = Digest32([0xCC; 32]);
    assert_ne!(r1.canonical_digest(), r2.canonical_digest());
}

#[test]
fn v2_digest_changes_when_any_field_changes() {
    // Spot-check several more fields not covered individually above.
    let base = dev_record();
    let mutations: &[fn(&mut AttestationRecordV2)] = &[
        |r| r.profile = AttestationProfile::FjellLocalV2Json,
        |r| r.boot.selected_slot = 1,
        |r| r.boot.kernel_digest = Digest32([0xFF; 32]),
        |r| r.verification.release_digest = Digest32([0xEE; 32]),
        |r| r.freshness.generation = 99,
        |r| r.freshness.nonce_class = NonceClass::OperatorTyped as u8,
        |r| r.snapshot.snapshot_digest = Digest32([0xDD; 32]),
        |r| r.health.status = 1,
        |r| r.rollback.trust_provider_counter_value = 777,
        |r| r.schema_version = 4,
    ];
    for m in mutations {
        let mut r = base;
        m(&mut r);
        assert_ne!(base.canonical_digest(), r.canonical_digest());
    }
}

// ── Signed-by descriptor exclusion ───────────────────────────────────────────

#[test]
fn signed_by_descriptor_not_in_record_digest() {
    // Changing `signed_by` must NOT change `record_digest`.
    let record = dev_record();
    let digest = record.canonical_digest();

    let sb1 = dev_signed_by();
    let mut sb2 = sb1;
    sb2.keyring_anchor_epoch = 99;

    // record_digest is computed from the record alone; signed_by is separate.
    assert_eq!(digest, record.canonical_digest()); // sanity
    // Two SignedAttestationRecordV2 with different signed_by share record_digest.
    let provider = dev_provider();
    let s1 = SignedAttestationRecordV2::sign(record, &provider, sb1).unwrap();
    let s2 = SignedAttestationRecordV2::sign(record, &provider, sb2).unwrap();
    assert_eq!(s1.record_digest, s2.record_digest);
}

// ── Round-trip ────────────────────────────────────────────────────────────────

#[test]
fn v2_sign_then_verify_round_trip() {
    let record = dev_record();
    let provider = dev_provider();
    let signed = SignedAttestationRecordV2::sign(record, &provider, dev_signed_by()).unwrap();
    assert!(signed.verify(&provider));
}

#[test]
fn v2_tampered_digest_fails_verify() {
    let record = dev_record();
    let provider = dev_provider();
    let mut signed = SignedAttestationRecordV2::sign(record, &provider, dev_signed_by()).unwrap();
    signed.record_digest.0[0] ^= 0xFF;
    assert!(!signed.verify(&provider));
}

// ── Profile-type checks ───────────────────────────────────────────────────────

#[test]
fn v2_profile_tag_is_0x21() {
    assert_eq!(AttestationProfile::FjellLocalV2Binary as u8, 0x21);
}

#[test]
fn v2_json_profile_tag_is_0x22() {
    assert_eq!(AttestationProfile::FjellLocalV2Json as u8, 0x22);
}

#[test]
fn v1_profile_tag_unchanged() {
    assert_eq!(AttestationProfile::FjellLocalV1Binary as u8, 0x01);
}

// ── Provenance coverage ───────────────────────────────────────────────────────

#[test]
fn v2_digest_differs_with_provenance() {
    let r1 = dev_record(); // no provenance
    let mut r2 = r1;
    r2.provenance = Some(ProvenanceClaims {
        sidecar_digest: Digest32([0x77; 32]),
        result: 0,
    });
    assert_ne!(r1.canonical_digest(), r2.canonical_digest());
}

// ── Nonce-class stability ────────────────────────────────────────────────────

#[test]
fn nonce_class_tags_stable() {
    assert_eq!(NonceClass::LocalOnly as u8, 0x01);
    assert_eq!(NonceClass::OperatorTyped as u8, 0x02);
    assert_eq!(NonceClass::RemoteChallenge as u8, 0x03);
}

// ── Schema version ───────────────────────────────────────────────────────────

#[test]
fn v2_schema_version_is_3() {
    // RFC-0.40-001: 2 -> 3, because availability flags are now in the stream.
    let r = dev_record();
    assert_eq!(r.schema_version, 3);
    assert_eq!(AttestationRecordV2::SCHEMA_VERSION, 3);
}

// ── RFC-0.40-001: the record says only what is true (closes E-094) ───────────

#[test]
fn the_local_record_asserts_no_verification_and_claims_no_unmeasured_digest() {
    let r = truthful_record();
    assert!(!r.verification.release_verified);
    assert!(!r.verification.rootfs_verified);
    assert!(!r.verification.policy_verified);
    assert_eq!(r.verified_flags(), 0, "the written flags byte is zero");
    let a = r.availability;
    assert!(!a.kernel_digest && !a.release_digest && !a.rootfs_digest);
    assert!(!a.policy_digest && !a.snapshot_digest);
    for d in [
        r.boot.kernel_digest,
        r.verification.release_digest,
        r.verification.rootfs_digest,
        r.verification.policy_digest,
        r.snapshot.snapshot_digest,
    ] {
        assert_eq!(d, Digest32([0u8; 32]), "a digest nothing computes is zero");
    }
}

#[test]
fn a_digest_behind_a_false_flag_does_not_count_even_if_the_caller_sets_it() {
    // The encoder zeroes; it does not trust the field. Setting a raw value on an
    // unavailable claim must not change what is signed.
    let base = truthful_record();
    let mutations: &[fn(&mut AttestationRecordV2)] = &[
        |r| r.boot.kernel_digest = Digest32([0xAA; 32]),
        |r| r.verification.release_digest = Digest32([0xAA; 32]),
        |r| r.verification.rootfs_digest = Digest32([0xAA; 32]),
        |r| r.verification.policy_digest = Digest32([0xAA; 32]),
        |r| r.snapshot.snapshot_digest = Digest32([0xAA; 32]),
    ];
    for m in mutations {
        let mut r = base;
        m(&mut r);
        assert_eq!(base.canonical_digest(), r.canonical_digest());
    }
}

#[test]
fn a_verified_claim_without_its_digest_is_not_written() {
    // `release_verified = true` beside an unavailable release digest would be a
    // verification of nothing; the encoder writes the bit as 0.
    let mut r = truthful_record();
    r.verification.release_verified = true;
    r.verification.rootfs_verified = true;
    r.verification.policy_verified = true;
    assert_eq!(r.verified_flags(), 0);
    assert_eq!(r.canonical_digest(), truthful_record().canonical_digest());
    // With the digest available the claim is written, and is covered.
    r.availability.release_digest = true;
    assert_eq!(r.verified_flags(), 0b001);
    assert_ne!(r.canonical_digest(), truthful_record().canonical_digest());
}

#[test]
fn availability_flags_are_covered_by_the_digest_each_on_its_own() {
    let base = dev_record();
    let flips: &[fn(&mut ClaimAvailability)] = &[
        |a| a.kernel_digest = false,
        |a| a.release_digest = false,
        |a| a.rootfs_digest = false,
        |a| a.policy_digest = false,
        |a| a.snapshot_digest = false,
        |a| a.measurement = false,
    ];
    for f in flips {
        let mut r = base;
        f(&mut r.availability);
        assert_ne!(base.canonical_digest(), r.canonical_digest());
    }
}

#[test]
fn an_unavailable_measurement_zeroes_the_whole_section() {
    // `measuredd` could not be reached: the section is unavailable and carries no
    // head, whatever the struct holds -- never a stale or literal one.
    let none = AttestationRecordV2::local(
        AttestationRecordId(*b"AT000001"),
        TrustProviderId::new(1),
        *b"stable\0\0",
        [0u8; 16],
        42,
        1,
        None,
    );
    assert!(!none.availability.measurement);
    let mut stale = none;
    stale.measurement = chain_reading();
    assert_eq!(
        none.canonical_digest(),
        stale.canonical_digest(),
        "a reading behind measurement.available = 0 must not count"
    );
    assert_ne!(
        none.canonical_digest(),
        truthful_record().canonical_digest(),
        "an unavailable section and a real one are different records"
    );
}

#[test]
fn the_real_measurement_is_covered_when_available() {
    let base = truthful_record();
    let mutations: &[fn(&mut AttestationRecordV2)] = &[
        |r| r.measurement.head_seq = 5,
        |r| r.measurement.chain_digest = Digest32([0x01; 32]),
        |r| r.measurement.included_from_seq = 2,
        |r| r.measurement.included_to_seq = 3,
    ];
    for m in mutations {
        let mut r = base;
        m(&mut r);
        assert_ne!(base.canonical_digest(), r.canonical_digest());
    }
}
