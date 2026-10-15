//! Host unit tests for `fjell-diag-format` (RFC v0.4-005 §11).

use crate::builder::{BuilderError, BundleBuilder};
use crate::bundle::DiagnosticBundle;
use crate::events::{
    AUDIT_KERNEL_BOOT_BANNER, AUDIT_NET_DRIVER_FAULTED, AUDIT_RECOVERY_ENTERED,
    AUDIT_SXT_HANDSHAKE_FAILED, AUDIT_UPGRADE_ROLLBACK_REJECTED, is_audit_event_allowed,
};
use crate::intents::{
    INTENT_NET_LINK_DOWN, INTENT_RECOVERY_ENTERED, INTENT_UPDATE_STAGING_CONFIRMED,
    INTENT_UPDATE_STAGING_FAILED, is_intent_allowed,
};
use crate::{DIAG_BUNDLE_VERSION, MAX_AUDIT_EVENTS, MAX_SEMANTIC_INTENTS};
use fjell_canon::BufSink;
use fjell_measure_format::Digest32;
use fjell_trust_provider::ids::TrustProviderId;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn default_builder() -> BundleBuilder {
    BundleBuilder::new(
        *b"test0001",
        42_000,
        TrustProviderId(1),
        Digest32([0xAAu8; 32]),
        true,
    )
}

// ── Allow-list tests ──────────────────────────────────────────────────────────

#[test]
fn audit_allow_list_accepts_known_tags() {
    assert!(is_audit_event_allowed(AUDIT_KERNEL_BOOT_BANNER));
    assert!(is_audit_event_allowed(AUDIT_UPGRADE_ROLLBACK_REJECTED));
    assert!(is_audit_event_allowed(AUDIT_SXT_HANDSHAKE_FAILED));
    assert!(is_audit_event_allowed(AUDIT_RECOVERY_ENTERED));
    assert!(is_audit_event_allowed(AUDIT_NET_DRIVER_FAULTED));
}

#[test]
fn audit_allow_list_rejects_unknown_tags() {
    // Tag 0x0001 is not on the allow-list.
    assert!(!is_audit_event_allowed(0x0001));
    // No all-ones tag.
    assert!(!is_audit_event_allowed(0xFFFF));
    // 0x00C0 is not in the list.
    assert!(!is_audit_event_allowed(0x00C0));
}

#[test]
fn intent_allow_list_accepts_known_tags() {
    assert!(is_intent_allowed(INTENT_UPDATE_STAGING_CONFIRMED));
    assert!(is_intent_allowed(INTENT_RECOVERY_ENTERED));
    assert!(is_intent_allowed(INTENT_NET_LINK_DOWN));
}

#[test]
fn intent_allow_list_rejects_unknown_tags() {
    assert!(!is_intent_allowed(0x0001));
    assert!(!is_intent_allowed(0xFFFF));
    assert!(!is_intent_allowed(0x0200));
}

// ── BundleBuilder — basic accumulation ───────────────────────────────────────

#[test]
fn builder_accepts_allowed_audit_event() {
    let mut b = default_builder();
    let r = b.add_audit_event(1, AUDIT_KERNEL_BOOT_BANNER, 0, 100);
    assert!(r.is_ok());
    assert_eq!(b.audit_count(), 1);
}

#[test]
fn builder_rejects_disallowed_audit_event() {
    let mut b = default_builder();
    let r = b.add_audit_event(1, 0xDEAD, 0, 100);
    assert_eq!(r, Err(BuilderError::NotAllowed));
    assert_eq!(b.audit_count(), 0);
}

#[test]
fn builder_accepts_allowed_intent() {
    let mut b = default_builder();
    let r = b.add_intent(1, INTENT_UPDATE_STAGING_FAILED, 0, 200);
    assert!(r.is_ok());
    assert_eq!(b.intent_count(), 1);
}

#[test]
fn builder_rejects_disallowed_intent() {
    let mut b = default_builder();
    let r = b.add_intent(1, 0x9999, 0, 200);
    assert_eq!(r, Err(BuilderError::NotAllowed));
}

// ── BundleBuilder — capacity limits ──────────────────────────────────────────

#[test]
fn builder_audit_buffer_full_returns_error() {
    let mut b = default_builder();
    for i in 0..MAX_AUDIT_EVENTS as u32 {
        b.add_audit_event(i, AUDIT_KERNEL_BOOT_BANNER, 0, i as u64)
            .unwrap();
    }
    assert_eq!(b.audit_count(), MAX_AUDIT_EVENTS as u8);
    let r = b.add_audit_event(MAX_AUDIT_EVENTS as u32, AUDIT_KERNEL_BOOT_BANNER, 0, 0);
    assert_eq!(r, Err(BuilderError::AuditFull));
}

#[test]
fn builder_intent_buffer_full_returns_error() {
    let mut b = default_builder();
    for i in 0..MAX_SEMANTIC_INTENTS as u32 {
        b.add_intent(i, INTENT_NET_LINK_DOWN, 0, i as u64).unwrap();
    }
    assert_eq!(b.intent_count(), MAX_SEMANTIC_INTENTS as u8);
    let r = b.add_intent(MAX_SEMANTIC_INTENTS as u32, INTENT_NET_LINK_DOWN, 0, 0);
    assert_eq!(r, Err(BuilderError::IntentFull));
}

// ── BundleBuilder — finalise ──────────────────────────────────────────────────

#[test]
fn builder_finalise_sets_schema_version() {
    let b = default_builder();
    let bundle = b.finalise();
    assert_eq!(bundle.schema_version, DIAG_BUNDLE_VERSION);
}

#[test]
fn builder_finalise_digest_is_nonzero() {
    let mut b = default_builder();
    b.add_audit_event(1, AUDIT_KERNEL_BOOT_BANNER, 0, 100)
        .unwrap();
    let bundle = b.finalise();
    assert_ne!(
        bundle.bundle_digest.0, [0u8; 32],
        "digest should be non-zero"
    );
}

#[test]
fn builder_finalise_digest_is_deterministic() {
    let add_events = |b: &mut BundleBuilder| {
        b.add_audit_event(1, AUDIT_KERNEL_BOOT_BANNER, 0, 100)
            .unwrap();
        b.add_intent(1, INTENT_UPDATE_STAGING_CONFIRMED, 0, 200)
            .unwrap();
    };
    let mut b1 = default_builder();
    add_events(&mut b1);
    let d1 = b1.finalise().bundle_digest;

    let mut b2 = default_builder();
    add_events(&mut b2);
    let d2 = b2.finalise().bundle_digest;

    assert_eq!(
        d1.0, d2.0,
        "identical bundles must produce identical digests"
    );
}

#[test]
fn builder_finalise_different_events_produce_different_digests() {
    let mut b1 = default_builder();
    b1.add_audit_event(1, AUDIT_KERNEL_BOOT_BANNER, 0, 100)
        .unwrap();
    let d1 = b1.finalise().bundle_digest;

    let mut b2 = default_builder();
    b2.add_audit_event(1, AUDIT_RECOVERY_ENTERED, 0, 100)
        .unwrap();
    let d2 = b2.finalise().bundle_digest;

    assert_ne!(
        d1.0, d2.0,
        "different events must produce different digests"
    );
}

#[test]
fn builder_preserves_event_fields_in_bundle() {
    let mut b = default_builder();
    b.add_audit_event(42, AUDIT_UPGRADE_ROLLBACK_REJECTED, 0x0007, 9999)
        .unwrap();
    let bundle = b.finalise();
    let ev = &bundle.audit_events[0];
    assert_eq!(ev.seq, 42);
    assert_eq!(ev.kind_tag, AUDIT_UPGRADE_ROLLBACK_REJECTED);
    assert_eq!(ev.code, 0x0007);
    assert_eq!(ev.at_tick, 9999);
}

// ── RFC-0.36-001: a field must say what it is (E-076, E-079, E-080) ─────────────

fn digest_of(b: &DiagnosticBundle) -> [u8; 32] {
    let mut sink = BufSink::<1700>::new();
    b.write_canonical(&mut sink);
    Digest32::of(sink.bytes()).0
}

/// RFC-v0.4-005 §8 names this test; it did not exist, and `write_canonical`
/// did not write `provider_id` or `keyring_anchor_epoch` (E-080). Two
/// bundles differing in exactly one of those fields must not share a digest.
#[test]
fn bundle_digest_covers_all_fields() {
    let base = default_builder().finalise();

    let mut changed_provider = base;
    changed_provider.provider_id = TrustProviderId(base.provider_id.0 + 1);
    assert_ne!(
        digest_of(&base),
        digest_of(&changed_provider),
        "provider_id must be covered by the digest"
    );

    let mut epoch_a = base;
    epoch_a.keyring_anchor_epoch_available = true;
    epoch_a.keyring_anchor_epoch = 99;
    let mut epoch_b = base;
    epoch_b.keyring_anchor_epoch_available = true;
    epoch_b.keyring_anchor_epoch = 100;
    assert_ne!(
        digest_of(&epoch_a),
        digest_of(&epoch_b),
        "keyring_anchor_epoch must be covered by the digest"
    );
}

/// RFC-0.36-001 R3: an availability flag must zero its field in the digest,
/// not merely accompany whatever the caller happened to store — the exact
/// shape of E-079's `last_attestation`/`keyring_anchor_epoch` fabrication,
/// now made unrepresentable in the encoder rather than trusted away.
#[test]
fn unavailable_fields_are_zeroed_not_the_callers_raw_value() {
    let mut a = DiagnosticBundle::zeroed();
    a.keyring_anchor_epoch_available = false;
    a.keyring_anchor_epoch = 123;
    let mut b = DiagnosticBundle::zeroed();
    b.keyring_anchor_epoch_available = false;
    b.keyring_anchor_epoch = 456;
    assert_eq!(
        digest_of(&a),
        digest_of(&b),
        "two different raw epoch values must hash identically while unavailable"
    );

    let mut c = DiagnosticBundle::zeroed();
    c.last_attestation_available = false;
    c.last_attestation = Digest32([0x77u8; 32]);
    let mut d = DiagnosticBundle::zeroed();
    d.last_attestation_available = false;
    d.last_attestation = Digest32([0x88u8; 32]);
    assert_eq!(
        digest_of(&c),
        digest_of(&d),
        "two different raw attestation digests must hash identically while unavailable"
    );

    let mut e = DiagnosticBundle::zeroed();
    e.measurement_head_available = false;
    e.measurement_head = Digest32([0x11u8; 32]);
    let mut f = DiagnosticBundle::zeroed();
    f.measurement_head_available = false;
    f.measurement_head = Digest32([0x22u8; 32]);
    assert_eq!(
        digest_of(&e),
        digest_of(&f),
        "two different raw measurement heads must hash identically while unavailable"
    );
}

/// RFC-0.36-001 (closes E-076): whatever sets `measurement_head_available`
/// true must be the code that obtained the value -- here, demonstrated as a
/// digest difference between the available and unavailable cases for the
/// identical underlying bytes.
#[test]
fn measurement_head_availability_changes_the_digest() {
    let same_bytes = Digest32([0x33u8; 32]);
    let mut available = DiagnosticBundle::zeroed();
    available.measurement_head_available = true;
    available.measurement_head = same_bytes;
    let mut unavailable = DiagnosticBundle::zeroed();
    unavailable.measurement_head_available = false;
    unavailable.measurement_head = same_bytes;
    assert_ne!(
        digest_of(&available),
        digest_of(&unavailable),
        "the same bytes must hash differently depending on availability"
    );
}

/// RFC-0.36-001 (closes E-079's third instance): `BundleBuilder::new` has no
/// parameter for `keyring_anchor_epoch` or `last_attestation` any more --
/// nothing obtains either value, so there is nothing honest to accept. Both
/// stay at `zeroed()`'s unavailable defaults through the builder.
#[test]
fn builder_cannot_be_asked_for_an_epoch_or_an_attestation_it_never_obtained() {
    let bundle = default_builder().finalise();
    assert!(!bundle.keyring_anchor_epoch_available);
    assert_eq!(bundle.keyring_anchor_epoch, 0);
    assert!(!bundle.last_attestation_available);
    assert_eq!(bundle.last_attestation.0, [0u8; 32]);
}
