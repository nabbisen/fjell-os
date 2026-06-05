//! Host unit tests for `fjell-trust-provider`.
//!
//! Source of truth: RFC-v0.3-001 §11.1 — *Host unit tests*.
//!
//! These tests validate the *design specification* (not just the code path):
//! the registry's one-way phase transition, the generation-tagged handle
//! semantics, the development provider's deterministic signature/seal
//! invariants, and the null provider's "always-rejects" contract.
//!
//! Every test name corresponds to a bullet in the RFC; deletions or renames
//! here are a design-tracking signal.

// Bring `alloc` in for the debug-format test and the Vec<descriptor> collect.
// `no_std` crates may still use `extern crate alloc` under cfg(test).
extern crate alloc;

use crate::descriptor::TrustProviderDescriptor;
use crate::development::DevelopmentTrustProvider;
use crate::error::TrustError;
use crate::ids::{KeyPurpose, ProviderHandle, TrustProviderId};
use crate::material::{AttestationDigest, KeyMaterial, SealedKey, Signature, SIGNATURE_LEN};
use crate::null::NullTrustProvider;
use crate::profile::{
    TrustProfile, TrustProviderCapabilities, TrustProviderKind, TrustProviderState,
};
use crate::provider::HardwareTrustProvider;
use crate::registry::{ProviderRegistry, RegistryError, RegistryPhase, MAX_PROVIDERS};

use fjell_measure_format::{Digest32, MeasurementHead, MeasurementKind};

// ---------------------------------------------------------------------------
// Helpers (test fixtures)
// ---------------------------------------------------------------------------

fn dev_descriptor() -> TrustProviderDescriptor {
    TrustProviderDescriptor::new(
        TrustProviderId::UNSET, // registry assigns
        TrustProviderKind::Development,
        TrustProfile::FjellLocalV1,
        TrustProviderCapabilities::DEVELOPMENT_BASELINE,
        TrustProviderState::Active,
        0, // registry assigns
        *b"fjell-dv",
    )
}

fn null_descriptor() -> TrustProviderDescriptor {
    TrustProviderDescriptor::new(
        TrustProviderId::UNSET,
        TrustProviderKind::Null,
        TrustProfile::FjellLocalV1,
        TrustProviderCapabilities::NONE,
        TrustProviderState::Active,
        0,
        *b"null---\0",
    )
}

fn digest_from_byte(b: u8) -> AttestationDigest {
    let mut bytes = [0u8; 32];
    bytes[0] = b;
    AttestationDigest(Digest32(bytes))
}

// ===========================================================================
// Section A — Identifier and Key-Purpose Stability
// ===========================================================================

#[test]
fn provider_id_unset_is_sentinel() {
    let unset = TrustProviderId::UNSET;
    assert!(unset.is_unset());
    assert_eq!(unset.0, 0);

    let real = TrustProviderId::new(1);
    assert!(!real.is_unset());
}

#[test]
fn provider_handle_default_is_unset() {
    let h = ProviderHandle::UNSET;
    assert!(h.is_unset());
    assert_eq!(h.id, TrustProviderId::UNSET);
    assert_eq!(h.generation, 0);
}

#[test]
fn key_purpose_tags_are_stable() {
    // Stability assertion: these byte values are projected into audit
    // records and persisted state. Changing them is a breaking change.
    assert_eq!(KeyPurpose::ReleaseVerification.tag(), 0x01);
    assert_eq!(KeyPurpose::RootfsVerification.tag(), 0x02);
    assert_eq!(KeyPurpose::PolicyVerification.tag(), 0x03);
    assert_eq!(KeyPurpose::AttestationSigning.tag(), 0x04);
    assert_eq!(KeyPurpose::SealedDataKey.tag(), 0x05);
    assert_eq!(KeyPurpose::SnapshotSigning.tag(), 0x06);
}

#[test]
fn key_purpose_verification_only_classification() {
    assert!(KeyPurpose::ReleaseVerification.is_verification_only());
    assert!(KeyPurpose::RootfsVerification.is_verification_only());
    assert!(KeyPurpose::PolicyVerification.is_verification_only());
    assert!(!KeyPurpose::AttestationSigning.is_verification_only());
    assert!(!KeyPurpose::SealedDataKey.is_verification_only());
    assert!(!KeyPurpose::SnapshotSigning.is_verification_only());
}

#[test]
fn descriptor_permitted_in_release_excludes_null() {
    assert!(dev_descriptor().permitted_in_release());
    assert!(!null_descriptor().permitted_in_release());
}

#[test]
fn capabilities_contains_and_union() {
    let read = TrustProviderCapabilities::READ_MEASUREMENT;
    let sign = TrustProviderCapabilities::SIGN_ATTESTATION;
    let both = read.union(sign);

    assert!(both.contains(read));
    assert!(both.contains(sign));
    assert!(!read.contains(sign));
    assert!(both.contains(both));

    let baseline = TrustProviderCapabilities::DEVELOPMENT_BASELINE;
    assert!(baseline.contains(read));
    assert!(baseline.contains(sign));
    assert!(baseline.contains(TrustProviderCapabilities::SEAL_KEY));
    assert!(baseline.contains(TrustProviderCapabilities::UNSEAL_KEY));
    assert!(baseline.contains(TrustProviderCapabilities::READ_ROLLBACK_COUNTER));
}

// ===========================================================================
// Section B — Registry: Bootstrap and Enforcing Phases
// ===========================================================================

#[test]
fn registry_starts_in_bootstrap() {
    let reg = ProviderRegistry::new();
    assert_eq!(reg.phase(), RegistryPhase::Bootstrap);
    assert!(reg.is_empty());
    assert_eq!(reg.len(), 0);
}

#[test]
fn registry_register_assigns_increasing_ids() {
    let mut reg = ProviderRegistry::new();
    let h1 = reg.register(dev_descriptor()).expect("first register");
    let h2 = reg.register(dev_descriptor()).expect("second register");
    let h3 = reg.register(dev_descriptor()).expect("third register");
    assert_ne!(h1.id, h2.id);
    assert_ne!(h2.id, h3.id);
    assert!(h1.id.0 < h2.id.0);
    assert!(h2.id.0 < h3.id.0);
    // generation starts at 1 (registry normalises 0 → 1)
    assert!(h1.generation >= 1);
    assert!(h2.generation >= 1);
}

#[test]
fn registry_lookup_returns_descriptor() {
    let mut reg = ProviderRegistry::new();
    let h = reg.register(dev_descriptor()).expect("register");
    let d = reg.lookup(h).expect("lookup");
    assert_eq!(d.id, h.id);
    assert_eq!(d.kind, TrustProviderKind::Development);
    assert_eq!(d.profile, TrustProfile::FjellLocalV1);
    assert_eq!(d.generation, h.generation);
}

#[test]
fn registry_lookup_rejects_unset_handle() {
    let reg = ProviderRegistry::new();
    let err = reg.lookup(ProviderHandle::UNSET).unwrap_err();
    assert_eq!(err, RegistryError::NotFound);
}

#[test]
fn registry_register_full_returns_capacity_exhausted() {
    let mut reg = ProviderRegistry::new();
    for _ in 0..MAX_PROVIDERS {
        reg.register(dev_descriptor()).expect("fill slot");
    }
    let err = reg.register(dev_descriptor()).unwrap_err();
    assert_eq!(err, RegistryError::CapacityExhausted);
    assert_eq!(reg.len(), MAX_PROVIDERS);
}

#[test]
fn registry_enter_enforcing_is_one_way() {
    let mut reg = ProviderRegistry::new();
    assert_eq!(reg.phase(), RegistryPhase::Bootstrap);
    reg.enter_enforcing();
    assert_eq!(reg.phase(), RegistryPhase::Enforcing);
    // Calling again is a no-op and must not panic.
    reg.enter_enforcing();
    assert_eq!(reg.phase(), RegistryPhase::Enforcing);
}

#[test]
fn registry_enforcing_rejects_null_provider() {
    let mut reg = ProviderRegistry::new();
    reg.enter_enforcing();
    let err = reg.register(null_descriptor()).unwrap_err();
    // The Null-specific rejection must precede the generic phase lock.
    assert_eq!(err, RegistryError::NullProviderForbidden);
}

#[test]
fn registry_enforcing_rejects_new_non_null_provider() {
    let mut reg = ProviderRegistry::new();
    reg.enter_enforcing();
    let err = reg.register(dev_descriptor()).unwrap_err();
    assert_eq!(err, RegistryError::PhaseLocked);
}

#[test]
fn registry_replace_rotates_generation() {
    let mut reg = ProviderRegistry::new();
    let h1 = reg.register(dev_descriptor()).expect("register");
    let h2 = reg.replace(h1, dev_descriptor()).expect("replace");
    assert_eq!(h1.id, h2.id);
    assert_ne!(h1.generation, h2.generation);
    assert!(h2.generation > h1.generation);
}

#[test]
fn registry_replace_in_enforcing_rejects_null() {
    let mut reg = ProviderRegistry::new();
    let h1 = reg.register(dev_descriptor()).expect("register");
    reg.enter_enforcing();
    let err = reg.replace(h1, null_descriptor()).unwrap_err();
    assert_eq!(err, RegistryError::NullProviderForbidden);
    // Non-null replace still works in Enforcing phase:
    let h2 = reg.replace(h1, dev_descriptor()).expect("non-null replace");
    assert_eq!(h1.id, h2.id);
    assert!(h2.generation > h1.generation);
}

#[test]
fn registry_remove_rotates_generation() {
    let mut reg = ProviderRegistry::new();
    let h = reg.register(dev_descriptor()).expect("register");
    reg.remove(h).expect("remove");
    // The slot is now free; further lookup must report NotFound.
    let err = reg.lookup(h).unwrap_err();
    assert_eq!(err, RegistryError::NotFound);
    assert_eq!(reg.len(), 0);
}

#[test]
fn registry_stale_handle_after_replace_rejected() {
    let mut reg = ProviderRegistry::new();
    let h1 = reg.register(dev_descriptor()).expect("register");
    let _h2 = reg.replace(h1, dev_descriptor()).expect("replace");
    // Old handle must now fail with StaleHandle.
    let err = reg.lookup(h1).unwrap_err();
    assert_eq!(err, RegistryError::StaleHandle);
}

#[test]
fn registry_stale_handle_after_remove_rejected() {
    let mut reg = ProviderRegistry::new();
    let h = reg.register(dev_descriptor()).expect("register");
    reg.remove(h).expect("remove");
    // After removal, the slot is empty; lookup is NotFound (the handle
    // can't be stale against a non-existent generation).
    let err = reg.lookup(h).unwrap_err();
    assert_eq!(err, RegistryError::NotFound);
}

#[test]
fn registry_descriptors_iterate_only_live_slots() {
    let mut reg = ProviderRegistry::new();
    let h1 = reg.register(dev_descriptor()).expect("register 1");
    let _h2 = reg.register(dev_descriptor()).expect("register 2");
    let h3 = reg.register(dev_descriptor()).expect("register 3");
    reg.remove(h1).expect("remove h1");
    let live: alloc::vec::Vec<TrustProviderDescriptor> = reg.descriptors().collect();
    assert_eq!(live.len(), 2);
    // Removed id must not appear.
    assert!(live.iter().all(|d| d.id != h1.id));
    // h3 must appear.
    assert!(live.iter().any(|d| d.id == h3.id));
}

// ===========================================================================
// Section C — DevelopmentTrustProvider Semantics
// ===========================================================================

#[test]
fn development_provider_signs_deterministically() {
    let p = DevelopmentTrustProvider::with_default_key(TrustProviderId::new(1), 1);
    let digest = digest_from_byte(0x42);
    let s1 = p.sign_attestation(digest).expect("sign 1");
    let s2 = p.sign_attestation(digest).expect("sign 2");
    assert_eq!(s1.bytes, s2.bytes);
    assert_eq!(s1.len, s2.len);
    assert!(s1.len > 0);
    assert!(s1.len as usize <= SIGNATURE_LEN);
}

#[test]
fn development_provider_signs_differ_per_input() {
    let p = DevelopmentTrustProvider::with_default_key(TrustProviderId::new(1), 1);
    let s1 = p.sign_attestation(digest_from_byte(0x10)).expect("sign a");
    let s2 = p.sign_attestation(digest_from_byte(0x11)).expect("sign b");
    assert_ne!(s1.bytes, s2.bytes);
}

#[test]
fn development_provider_signs_differ_per_provider_id() {
    let p1 = DevelopmentTrustProvider::with_default_key(TrustProviderId::new(1), 1);
    let p2 = DevelopmentTrustProvider::with_default_key(TrustProviderId::new(2), 1);
    let d = digest_from_byte(0xAA);
    let s1 = p1.sign_attestation(d).expect("sign p1");
    let s2 = p2.sign_attestation(d).expect("sign p2");
    assert_ne!(s1.bytes, s2.bytes);
}

#[test]
fn development_provider_sign_unsign_round_trip() {
    // RFC-v0.3-001 §11.1: "round-trip seal/unseal recovers the original bytes"
    let p = DevelopmentTrustProvider::with_default_key(TrustProviderId::new(7), 1);
    let original = KeyMaterial::from_bytes(&[
        0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04, //
        0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C,
    ]);
    let sealed = p
        .seal_key(KeyPurpose::SealedDataKey, original)
        .expect("seal");
    assert_eq!(sealed.purpose, KeyPurpose::SealedDataKey);
    let unsealed = p
        .unseal_key(KeyPurpose::SealedDataKey, &sealed)
        .expect("unseal");
    assert_eq!(unsealed.as_slice(), original.as_slice());
    assert_eq!(unsealed.len, original.len);
}

#[test]
fn development_provider_seal_unseal_zero_length_succeeds() {
    let p = DevelopmentTrustProvider::with_default_key(TrustProviderId::new(1), 1);
    let empty = KeyMaterial::ZERO;
    let sealed = p
        .seal_key(KeyPurpose::SealedDataKey, empty)
        .expect("seal empty");
    let unsealed = p
        .unseal_key(KeyPurpose::SealedDataKey, &sealed)
        .expect("unseal empty");
    assert_eq!(unsealed.len, 0);
    assert_eq!(unsealed.as_slice(), &[] as &[u8]);
}

#[test]
fn development_provider_sign_wrong_purpose_rejected() {
    let p = DevelopmentTrustProvider::with_default_key(TrustProviderId::new(1), 1);
    let key = KeyMaterial::from_bytes(b"secret-material");
    let sealed = p
        .seal_key(KeyPurpose::SealedDataKey, key)
        .expect("seal data key");
    // Asking to unseal under a different purpose must fail with the
    // specific PurposeMismatch error code (not generic Internal).
    let err = p
        .unseal_key(KeyPurpose::AttestationSigning, &sealed)
        .unwrap_err();
    assert_eq!(err, TrustError::PurposeMismatch);
}

#[test]
fn development_provider_corrupted_blob_fails_mac() {
    let p = DevelopmentTrustProvider::with_default_key(TrustProviderId::new(1), 1);
    let key = KeyMaterial::from_bytes(b"the-original");
    let mut sealed = p
        .seal_key(KeyPurpose::SealedDataKey, key)
        .expect("seal");
    // Flip a bit inside the MAC region (first 32 bytes).
    sealed.blob[3] ^= 0x01;
    let err = p
        .unseal_key(KeyPurpose::SealedDataKey, &sealed)
        .unwrap_err();
    assert_eq!(err, TrustError::SealIntegrityFailed);
}

#[test]
fn development_provider_corrupted_payload_fails_mac() {
    let p = DevelopmentTrustProvider::with_default_key(TrustProviderId::new(1), 1);
    let key = KeyMaterial::from_bytes(b"the-original");
    let mut sealed = p
        .seal_key(KeyPurpose::SealedDataKey, key)
        .expect("seal");
    // Flip a bit inside the payload region (byte index 32+).
    sealed.blob[40] ^= 0x80;
    let err = p
        .unseal_key(KeyPurpose::SealedDataKey, &sealed)
        .unwrap_err();
    assert_eq!(err, TrustError::SealIntegrityFailed);
}

#[test]
fn development_provider_rollback_counter_monotonic() {
    let p = DevelopmentTrustProvider::with_default_key(TrustProviderId::new(1), 1);
    let c0 = p.read_anti_rollback_counter().expect("read 0");
    let c1 = p.advance_rollback_counter().expect("advance 1");
    let c2 = p.advance_rollback_counter().expect("advance 2");
    let c3 = p.read_anti_rollback_counter().expect("read 3");
    assert!(c1 > c0);
    assert!(c2 > c1);
    assert_eq!(c2, c3); // read returns current, not advance
}

#[test]
fn development_provider_faulted_rejects_all() {
    let p = DevelopmentTrustProvider::with_default_key(TrustProviderId::new(1), 1);
    p.force_fault();

    // Every capability-bearing method must return ProviderUnavailable.
    assert_eq!(
        p.read_measurement().unwrap_err(),
        TrustError::ProviderUnavailable
    );
    assert_eq!(
        p.sign_attestation(AttestationDigest::ZERO).unwrap_err(),
        TrustError::ProviderUnavailable
    );
    assert_eq!(
        p.read_anti_rollback_counter().unwrap_err(),
        TrustError::ProviderUnavailable
    );
    assert_eq!(
        p.seal_key(KeyPurpose::SealedDataKey, KeyMaterial::ZERO)
            .unwrap_err(),
        TrustError::ProviderUnavailable
    );
    let empty_sealed = SealedKey::empty(KeyPurpose::SealedDataKey);
    assert_eq!(
        p.unseal_key(KeyPurpose::SealedDataKey, &empty_sealed)
            .unwrap_err(),
        TrustError::ProviderUnavailable
    );

    // Descriptor & provider_id remain readable in Faulted state — they are
    // metadata, not capability calls.
    assert_eq!(p.descriptor().state, TrustProviderState::Faulted);
    assert!(!p.descriptor().is_usable());
}

#[test]
fn development_provider_set_measurement_visible() {
    let p = DevelopmentTrustProvider::with_default_key(TrustProviderId::new(1), 1);
    let mut chain = [0u8; 32];
    chain[0] = 0xC0;
    chain[1] = 0xDE;
    let head = MeasurementHead {
        latest_seq: 7,
        chain_digest: Digest32(chain),
        dropped: 0,
        last_event_kind: MeasurementKind::BootEvidenceImported,
    };
    p.set_measurement(head);
    let read = p.read_measurement().expect("read");
    assert_eq!(read.latest_seq, 7);
    assert_eq!(read.chain_digest.0[0], 0xC0);
    assert_eq!(read.chain_digest.0[1], 0xDE);
}

// ===========================================================================
// Section D — NullTrustProvider Contract
// ===========================================================================

#[test]
fn null_provider_returns_not_supported_everywhere() {
    let p = NullTrustProvider::new(TrustProviderId::new(99), 1);
    assert_eq!(p.descriptor().kind, TrustProviderKind::Null);
    assert!(!p.descriptor().permitted_in_release());

    // All capability methods must return NotSupported (the trait default).
    assert_eq!(p.read_measurement().unwrap_err(), TrustError::NotSupported);
    assert_eq!(
        p.sign_attestation(AttestationDigest::ZERO).unwrap_err(),
        TrustError::NotSupported
    );
    assert_eq!(
        p.read_anti_rollback_counter().unwrap_err(),
        TrustError::NotSupported
    );
    assert_eq!(
        p.seal_key(KeyPurpose::SealedDataKey, KeyMaterial::ZERO)
            .unwrap_err(),
        TrustError::NotSupported
    );
    let empty = SealedKey::empty(KeyPurpose::SealedDataKey);
    assert_eq!(
        p.unseal_key(KeyPurpose::SealedDataKey, &empty).unwrap_err(),
        TrustError::NotSupported
    );
}

// ===========================================================================
// Section E — Signature / KeyMaterial type contracts
// ===========================================================================

#[test]
fn signature_from_bytes_roundtrip() {
    let src = [0xAB; 16];
    let sig = Signature::from_bytes(&src);
    assert_eq!(sig.len, 16);
    assert_eq!(sig.as_slice(), &src);
    // Bytes beyond `len` must be zero.
    assert_eq!(sig.bytes[16], 0);
    assert_eq!(sig.bytes[SIGNATURE_LEN - 1], 0);
}

#[test]
fn signature_from_bytes_truncates_oversize() {
    let src = [0xCC; SIGNATURE_LEN + 32];
    let sig = Signature::from_bytes(&src);
    assert_eq!(sig.len as usize, SIGNATURE_LEN);
}

#[test]
fn keymaterial_debug_does_not_leak_bytes() {
    let secret = KeyMaterial::from_bytes(b"super-secret-key-material");
    let printed = alloc::format!("{:?}", secret);
    assert!(printed.contains("REDACTED"));
    assert!(!printed.contains("super-secret"));
    assert!(!printed.contains("key-material"));
}

#[test]
fn keymaterial_eq_compares_only_meaningful_slice() {
    let a = KeyMaterial::from_bytes(&[1, 2, 3]);
    let b = KeyMaterial::from_bytes(&[1, 2, 3]);
    assert_eq!(a, b);
    let c = KeyMaterial::from_bytes(&[1, 2, 3, 4]);
    assert_ne!(a, c);
}

#[test]
fn trust_error_codes_are_stable() {
    // Audit/semantic-stream projection relies on these numeric values.
    assert_eq!(TrustError::NotSupported.code(), 0x0001);
    assert_eq!(TrustError::ProviderUnavailable.code(), 0x0002);
    assert_eq!(TrustError::NullProviderForbidden.code(), 0x0003);
    assert_eq!(TrustError::StaleHandle.code(), 0x0004);
    assert_eq!(TrustError::PurposeMismatch.code(), 0x0005);
    assert_eq!(TrustError::SealIntegrityFailed.code(), 0x0006);
    assert_eq!(TrustError::RollbackCounterExhausted.code(), 0x0007);
    assert_eq!(TrustError::KeyMaterialTooLarge.code(), 0x0008);
    assert_eq!(TrustError::SignFailed.code(), 0x0009);
    assert_eq!(TrustError::Internal.code(), 0xFFFF);
}

// Bring `alloc` in for the debug-format test.  `no_std` crates may still use
// `extern crate alloc` under cfg(test) on a hosted target.  (moved to top.)
