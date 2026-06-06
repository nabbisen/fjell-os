//! Host unit tests for `fjell-diag-format` (RFC v0.4-005 §11).

use crate::bundle::{DiagnosticBundle, DIAG_BUNDLE_VERSION, MAX_AUDIT_EVENTS, MAX_SEMANTIC_INTENTS};
use crate::builder::{BundleBuilder, BuilderError};
use crate::events::{
    is_audit_event_allowed,
    AUDIT_KERNEL_BOOT_BANNER, AUDIT_UPGRADE_ROLLBACK_REJECTED,
    AUDIT_SXT_HANDSHAKE_FAILED, AUDIT_RECOVERY_ENTERED, AUDIT_NET_DRIVER_FAULTED,
};
use crate::intents::{
    is_intent_allowed,
    INTENT_UPDATE_STAGING_CONFIRMED, INTENT_RECOVERY_ENTERED,
    INTENT_NET_LINK_DOWN, INTENT_UPDATE_STAGING_FAILED,
};
use fjell_measure_format::Digest32;
use fjell_trust_provider::ids::TrustProviderId;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn default_builder() -> BundleBuilder {
    BundleBuilder::new(
        *b"test0001",
        42_000,
        TrustProviderId(1),
        7,
        Digest32([0xAAu8; 32]),
        Digest32([0xBBu8; 32]),
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
        b.add_audit_event(i, AUDIT_KERNEL_BOOT_BANNER, 0, i as u64).unwrap();
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
    b.add_audit_event(1, AUDIT_KERNEL_BOOT_BANNER, 0, 100).unwrap();
    let bundle = b.finalise();
    assert_ne!(bundle.bundle_digest.0, [0u8; 32], "digest should be non-zero");
}

#[test]
fn builder_finalise_digest_is_deterministic() {
    let add_events = |b: &mut BundleBuilder| {
        b.add_audit_event(1, AUDIT_KERNEL_BOOT_BANNER, 0, 100).unwrap();
        b.add_intent(1, INTENT_UPDATE_STAGING_CONFIRMED, 0, 200).unwrap();
    };
    let mut b1 = default_builder();
    add_events(&mut b1);
    let d1 = b1.finalise().bundle_digest;

    let mut b2 = default_builder();
    add_events(&mut b2);
    let d2 = b2.finalise().bundle_digest;

    assert_eq!(d1.0, d2.0, "identical bundles must produce identical digests");
}

#[test]
fn builder_finalise_different_events_produce_different_digests() {
    let mut b1 = default_builder();
    b1.add_audit_event(1, AUDIT_KERNEL_BOOT_BANNER, 0, 100).unwrap();
    let d1 = b1.finalise().bundle_digest;

    let mut b2 = default_builder();
    b2.add_audit_event(1, AUDIT_RECOVERY_ENTERED, 0, 100).unwrap();
    let d2 = b2.finalise().bundle_digest;

    assert_ne!(d1.0, d2.0, "different events must produce different digests");
}

#[test]
fn builder_preserves_event_fields_in_bundle() {
    let mut b = default_builder();
    b.add_audit_event(42, AUDIT_UPGRADE_ROLLBACK_REJECTED, 0x0007, 9999).unwrap();
    let bundle = b.finalise();
    let ev = &bundle.audit_events[0];
    assert_eq!(ev.seq,      42);
    assert_eq!(ev.kind_tag, AUDIT_UPGRADE_ROLLBACK_REJECTED);
    assert_eq!(ev.code,     0x0007);
    assert_eq!(ev.at_tick,  9999);
}
