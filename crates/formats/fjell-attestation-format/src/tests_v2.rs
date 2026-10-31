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
        Some(7),
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
        keyring_epochs: true,
        snapshot_id: true,
        health_status: true,
        last_advance_source: true,
        freshness_key_epoch: true,
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
        |r| r.schema_version = 5,
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
fn v2_schema_version_is_4() {
    // RFC-0.40-001: 2 -> 3, because availability flags are now in the stream.
    // RFC-0.41-003 R1: 3 -> 4, five more.
    let r = dev_record();
    assert_eq!(r.schema_version, 4);
    assert_eq!(AttestationRecordV2::SCHEMA_VERSION, 4);
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
    // RFC-0.41-003 R1: and none of the placeholders is claimed either.
    assert!(!a.keyring_epochs && !a.snapshot_id && !a.health_status);
    assert!(!a.last_advance_source && !a.freshness_key_epoch);
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
        |a| a.keyring_epochs = false,
        |a| a.snapshot_id = false,
        |a| a.health_status = false,
        |a| a.last_advance_source = false,
        |a| a.freshness_key_epoch = false,
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
        Some(7),
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

#[test]
fn the_counter_pair_reports_the_provider_read_and_not_the_rollback_floor() {
    // `truthful_record` has a rollback floor of 42 and a provider that answered 7.
    // The pair says what the provider said: a floor is a different quantity.
    let r = truthful_record();
    assert_eq!(r.rollback.min_counter, 42);
    assert!(r.rollback.trust_provider_counter_supported);
    assert_eq!(r.rollback.trust_provider_counter_value, 7);
}

#[test]
fn an_unreadable_counter_claims_no_support_and_no_value() {
    // The read FAILED (the null provider, or a development provider whose
    // descriptor is not usable): `false` and 0 -- not 0 beside a `true`, and not
    // the rollback floor.
    let r = AttestationRecordV2::local(
        AttestationRecordId(*b"AT000001"),
        TrustProviderId::new(1),
        *b"stable\0\0",
        [0u8; 16],
        42,
        None,
        1,
        Some(chain_reading()),
    );
    assert!(!r.rollback.trust_provider_counter_supported);
    assert_eq!(r.rollback.trust_provider_counter_value, 0);
    // And it is a different record from one whose read returned 0.
    let zero = AttestationRecordV2::local(
        AttestationRecordId(*b"AT000001"),
        TrustProviderId::new(1),
        *b"stable\0\0",
        [0u8; 16],
        42,
        Some(0),
        1,
        Some(chain_reading()),
    );
    assert_ne!(r.canonical_digest(), zero.canonical_digest());
}

// ── RFC-0.41-003 R2/R3: the ENCODER zeroes a flag-and-value pair, not the factory ──

/// The bytes `write_canonical` emits for `r`.
fn canonical_bytes(r: &AttestationRecordV2) -> alloc::vec::Vec<u8> {
    use fjell_canon::BufSink;
    let mut sink = BufSink::<{ crate::v2::ATTEST_V2_STREAM_MAX }>::new();
    r.write_canonical(&mut sink);
    sink.bytes().to_vec()
}

/// `AttestationRecordV2::local` is the only *honest* way to build a record, not the only
/// way: `RollbackClaims` and the rest are `pub` with `pub` fields. So the property -- a
/// value behind a `false` flag is written as zero -- must hold of the **encoder**, and this
/// test builds the record the way a caller who is not `local()` would: by assigning fields,
/// putting a deliberately non-zero value behind every false flag. If a zeroing is ever moved
/// back into a factory this is the test that fails; one that goes through `local()` cannot.
#[test]
fn every_flag_and_value_pair_is_zeroed_by_the_encoder_not_by_the_factory() {
    // The reference: every optional claim unavailable, built honestly.
    let honest = AttestationRecordV2::local(
        AttestationRecordId(*b"AT000001"),
        TrustProviderId::new(1),
        *b"stable\0\0",
        [0u8; 16],
        42,
        None, // the counter read failed: flag false, value 0
        1,
        None, // the chain could not be read
    );
    assert!(!honest.rollback.trust_provider_counter_supported);
    assert_eq!(honest.availability, ClaimAvailability::default());

    // The same record, built by FIELD ASSIGNMENT with a non-zero value behind every false flag.
    let mut r = honest;
    r.boot.kernel_digest = Digest32([0x11; 32]);
    r.verification.release_digest = Digest32([0x22; 32]);
    r.verification.rootfs_digest = Digest32([0x33; 32]);
    r.verification.policy_digest = Digest32([0x44; 32]);
    r.snapshot.snapshot_digest = Digest32([0x66; 32]);
    r.measurement = MeasurementClaims {
        head_seq: 9,
        chain_digest: Digest32([0x5C; 32]),
        included_from_seq: 3,
        included_to_seq: 9,
    };
    r.rollback.trust_provider_counter_value = 777; // beside supported == false
    // RFC-0.41-003 R1: the five placeholder pairs, a non-zero value behind each false flag
    // (a non-zero reason and status included: 0 is a valid one of each, so only a non-zero
    // value shows the encoder zeroed it).
    r.keyring.active_epoch_attestation = 0x0101_0101;
    r.keyring.active_epoch_release = 0x0202_0202;
    r.keyring.active_epoch_policy = 0x0303_0303;
    r.snapshot.snapshot_id = *b"SN999999";
    r.snapshot.reason = 5;
    r.health.status = 4;
    r.rollback.last_advance_source = 2;
    r.freshness.key_epoch = 0x0404_0404;
    assert_ne!(
        r, honest,
        "the fixture must actually differ from the reference"
    );

    assert_eq!(
        canonical_bytes(&r),
        canonical_bytes(&honest),
        "a value behind a false flag reached the encoded bytes: some pair is zeroed by its \
         factory, not by the encoder"
    );

    // The control that lets the assertion above fail: with each flag TRUE the same
    // values are written, so the stream differs. (Without this, equal streams could
    // mean the fields were never encoded at all.)
    type Set = fn(&mut AttestationRecordV2);
    let flips: [(&str, Set); 12] = [
        ("kernel_digest", |r| r.availability.kernel_digest = true),
        ("release_digest", |r| r.availability.release_digest = true),
        ("rootfs_digest", |r| r.availability.rootfs_digest = true),
        ("policy_digest", |r| r.availability.policy_digest = true),
        ("snapshot_digest", |r| r.availability.snapshot_digest = true),
        ("measurement", |r| r.availability.measurement = true),
        ("trust_provider_counter", |r| {
            r.rollback.trust_provider_counter_supported = true
        }),
        ("keyring_epochs", |r| r.availability.keyring_epochs = true),
        ("snapshot_id", |r| r.availability.snapshot_id = true),
        ("health_status", |r| r.availability.health_status = true),
        ("last_advance_source", |r| {
            r.availability.last_advance_source = true
        }),
        ("freshness_key_epoch", |r| {
            r.availability.freshness_key_epoch = true
        }),
    ];
    for (name, flip) in flips {
        let mut on = r;
        flip(&mut on);
        assert_ne!(
            canonical_bytes(&on),
            canonical_bytes(&honest),
            "turning `{name}` on did not change the stream: its value is not encoded"
        );
    }
}
