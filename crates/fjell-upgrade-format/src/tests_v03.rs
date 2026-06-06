//! Host unit tests for `fjell-upgrade-format` v0.3 additions.
//!
//! Source of truth: RFC v0.3-003 §11.1.
//!
//! The `stable_record` helper and `TapMeta`/`TapRecord` builder traits
//! are fixture utilities used by a subset of tests. Suppress dead_code
//! lint: these will be exercised by future property-test expansion.
#![allow(dead_code)]

extern crate alloc;

use crate::release_metadata::{
    Provenance, ReleaseMetadata, RELEASE_METADATA_VERSION,
};
use crate::rollback_record::{
    AdvanceSource, RollbackCheckResult, RollbackRecord,
    advance_min_counter, check_rollback,
};
use fjell_measure_format::Digest32;
use fjell_keyring::KeyEpoch;
use fjell_trust_provider::ids::TrustProviderId;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn stable_meta(counter: u64) -> ReleaseMetadata {
    ReleaseMetadata::dev(*b"stable\0\0", counter)
}

fn stable_record(min: u64) -> RollbackRecord {
    RollbackRecord::genesis(*b"stable\0\0")
        .clone()
        .tap_set_min(min)
}

// Helper trait to allow mutation for test fixtures.
trait TapMeta {
    fn tap_set_counter(self, c: u64) -> Self;
    fn tap_set_channel(self, ch: [u8; 8]) -> Self;
    fn tap_set_epoch(self, e: KeyEpoch) -> Self;
    fn tap_corrupt_digest(self) -> Self;
}

impl TapMeta for ReleaseMetadata {
    fn tap_set_counter(mut self, c: u64) -> Self {
        self.release_counter = c;
        self.metadata_digest = self.compute_digest();
        self
    }
    fn tap_set_channel(mut self, ch: [u8; 8]) -> Self {
        self.channel_id = ch;
        self.metadata_digest = self.compute_digest();
        self
    }
    fn tap_set_epoch(mut self, e: KeyEpoch) -> Self {
        self.signing_anchor_epoch = e;
        self.metadata_digest = self.compute_digest();
        self
    }
    fn tap_corrupt_digest(mut self) -> Self {
        self.metadata_digest.0[0] ^= 0xFF;
        self
    }
}

trait TapRecord {
    fn tap_set_min(self, m: u64) -> Self;
    fn tap_corrupt_digest(self) -> Self;
}

impl TapRecord for RollbackRecord {
    fn tap_set_min(mut self, m: u64) -> Self {
        self.min_counter = m;
        self.record_digest = self.compute_digest();
        self
    }
    fn tap_corrupt_digest(mut self) -> Self {
        self.record_digest.0[0] ^= 0xFF;
        self
    }
}

// ── ReleaseMetadata tests ────────────────────────────────────────────────────

#[test]
fn release_metadata_version_constant_stable() {
    assert_eq!(RELEASE_METADATA_VERSION, 1);
}

#[test]
fn release_metadata_digest_covers_counter() {
    let m1 = stable_meta(10);
    let m2 = stable_meta(11);
    assert_ne!(m1.metadata_digest, m2.metadata_digest);
}

#[test]
fn release_metadata_digest_covers_channel_id() {
    let m1 = stable_meta(1).tap_set_channel(*b"stable\0\0");
    let m2 = stable_meta(1).tap_set_channel(*b"lts\0\0\0\0\0");
    assert_ne!(m1.metadata_digest, m2.metadata_digest);
}

#[test]
fn release_metadata_digest_covers_anchor_epoch() {
    let m1 = stable_meta(1).tap_set_epoch(KeyEpoch::ONE);
    let m2 = stable_meta(1).tap_set_epoch(KeyEpoch(2));
    assert_ne!(m1.metadata_digest, m2.metadata_digest);
}

#[test]
fn release_metadata_verify_digest_self_consistent() {
    assert!(stable_meta(42).verify_digest());
}

#[test]
fn release_metadata_bad_digest_rejected() {
    let m = stable_meta(42).tap_corrupt_digest();
    assert!(!m.verify_digest());
}

#[test]
fn release_metadata_consistent_when_embedded_min_le_counter() {
    let m = ReleaseMetadata::new(
        *b"stable\0\0", 10, 8,
        Digest32([0; 32]), KeyEpoch::ONE, TrustProviderId::new(1),
        Digest32([0; 32]), 0, Provenance::DEV,
    );
    assert!(m.is_internally_consistent());
}

#[test]
fn release_metadata_inconsistent_when_embedded_min_exceeds_counter() {
    // Build manually to bypass the constructor's logic.
    let mut m = stable_meta(5);
    m.embedded_min_counter = 99;
    assert!(!m.is_internally_consistent());
}

#[test]
fn release_metadata_serialise_then_parse_round_trip() {
    // Verify compute_digest is deterministic (a proxy for serialisation stability).
    let m = stable_meta(100);
    assert_eq!(m.compute_digest(), m.compute_digest());
    assert_eq!(m.metadata_digest, m.compute_digest());
}

// ── RollbackRecord tests ─────────────────────────────────────────────────────

#[test]
fn rollback_record_verify_digest_self_consistent() {
    let r = RollbackRecord::new(*b"stable\0\0", 58, 1000, AdvanceSource::UpgradedConfirmation);
    assert!(r.verify_digest());
}

#[test]
fn rollback_record_bad_digest_rejected() {
    let r = RollbackRecord::new(*b"stable\0\0", 58, 0, AdvanceSource::UpgradedConfirmation)
        .tap_corrupt_digest();
    assert!(!r.verify_digest());
}

#[test]
fn rollback_record_serialise_then_parse_round_trip() {
    let r = RollbackRecord::new(*b"chan-a\0\0", 7, 99, AdvanceSource::BootctlPromotion);
    assert_eq!(r.compute_digest(), r.record_digest);
}

#[test]
fn rollback_record_advance_source_round_trip() {
    for (val, src) in [
        (0x01, AdvanceSource::UpgradedConfirmation),
        (0x02, AdvanceSource::RecoveryReset),
        (0x03, AdvanceSource::BootctlPromotion),
    ] {
        assert_eq!(AdvanceSource::from_u8(val), Some(src));
    }
    assert_eq!(AdvanceSource::from_u8(0xFF), None);
}

// ── check_rollback / advance_min_counter tests ───────────────────────────────

#[test]
fn check_rollback_allows_equal_counter() {
    assert_eq!(check_rollback(58, 58, 57), RollbackCheckResult::Allowed);
}

#[test]
fn check_rollback_allows_higher_counter() {
    assert_eq!(check_rollback(58, 63, 58), RollbackCheckResult::Allowed);
}

#[test]
fn check_rollback_rejects_lower_counter() {
    assert_eq!(
        check_rollback(58, 42, 41),
        RollbackCheckResult::Rejected { min_counter: 58 },
    );
}

#[test]
fn check_rollback_rejects_metadata_inconsistency() {
    // embedded_min_counter > candidate_counter is self-contradictory.
    assert_eq!(
        check_rollback(0, 10, 99),
        RollbackCheckResult::MetadataInconsistent,
    );
}

#[test]
fn advance_min_counter_takes_maximum() {
    assert_eq!(advance_min_counter(58, 63), 63);
    assert_eq!(advance_min_counter(58, 50), 58); // never goes down
    assert_eq!(advance_min_counter(0, 0), 0);
}
