//! Host unit tests for `fjell-identity-format` (RFC v0.7-001 §11,
//! updated for RFC-v0.7.2-003 API changes).

use crate::identity::{
    NodeIdentity, NodeIdentityBuilder, NodeId, NodeAlias, AttestationPubkey,
    IdentityError, NODE_IDENTITY_SCHEMA_VERSION, STORE_RECORD_KIND_IDENTITY,
};
use crate::policy::{NodeIdentityPolicy, TrustMode, Decision, PolicyError, RosterRef};
use crate::digest::identity_digest;
use fjell_measure_format::Digest32;

fn dummy_builder() -> NodeIdentityBuilder {
    NodeIdentityBuilder {
        node_id:            NodeId([1u8; 16]),
        alias:              NodeAlias(*b"test-node-00\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0"),
        created_tick:       1_000_000,
        trust_provider_id:  42,
        trust_profile_tag:  0x01,
        attestation_pubkey: AttestationPubkey([0xABu8; 32]),
        platform_digest:    Digest32([0x11u8; 32]),
        board_digest:       Digest32([0x22u8; 32]),
    }
}

fn dummy_identity() -> NodeIdentity {
    NodeIdentity::new(
        NodeId([1u8; 16]),
        NodeAlias(*b"test-node-00\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0"),
        1_000_000, 42, 0x01,
        AttestationPubkey([0xABu8; 32]),
        Digest32([0x11u8; 32]),
        Digest32([0x22u8; 32]),
    )
}

// ── Schema constants ───────────────────────────────────────────────────────────

#[test]
fn schema_version_is_one() { assert_eq!(NODE_IDENTITY_SCHEMA_VERSION, 1); }

#[test]
fn store_record_kind_identity_value() { assert_eq!(STORE_RECORD_KIND_IDENTITY, 0x0020); }

// ── NodeAlias ─────────────────────────────────────────────────────────────────

#[test]
fn node_alias_try_as_str_trims_at_nul() {
    let mut a = NodeAlias([0u8; 32]);
    b"my-node-1".iter().enumerate().for_each(|(i, &b)| a.0[i] = b);
    assert_eq!(a.try_as_str().unwrap(), "my-node-1");
    assert_eq!(NodeAlias([0u8; 32]).try_as_str().unwrap(), "");
}

#[test]
fn node_alias_try_as_str_invalid_utf8_returns_err() {
    let mut a = NodeAlias([0u8; 32]);
    a.0[0] = 0xFF; // invalid UTF-8 byte
    assert!(a.try_as_str().is_err());
}

#[test]
fn node_alias_as_str_lossy_handles_invalid() {
    let mut a = NodeAlias([0u8; 32]);
    a.0[0] = 0xFF;
    assert!(!a.as_str_lossy().is_empty());
    // Returns the marker string, not empty
    assert_eq!(a.as_str_lossy(), "<invalid-utf8-alias>");
}

// ── NodeIdentity safe constructor (RFC-v0.7.2-003) ────────────────────────────

// IDENTITY:NEW_WITH_DIGEST_NONZERO
#[test]
fn identity_build_produces_nonzero_digest() {
    let n = NodeIdentity::build(dummy_builder()).unwrap();
    assert_ne!(n.identity_digest.0, [0u8; 32]);
}

#[test]
fn identity_build_validates_on_reload() {
    let n = NodeIdentity::build(dummy_builder()).unwrap();
    assert!(n.validate_digest().is_ok());
}

// IDENTITY:ZERO_DIGEST_REJECTED_ON_LOAD
#[test]
fn identity_legacy_new_has_zero_digest() {
    let n = dummy_identity();
    assert_eq!(n.identity_digest.0, [0u8; 32]);
    // validate_digest fails because digest was never computed
    assert_eq!(n.validate_digest(), Err(IdentityError::DigestMismatch));
}

#[test]
fn identity_digest_matches_after_manual_compute() {
    let mut n = dummy_identity();
    n.identity_digest = identity_digest(&n);
    assert!(n.validate_digest().is_ok());
}

// ── NodeIdentityPolicy (RFC-v0.7.2-003) ───────────────────────────────────────

#[test]
fn same_family_policy_allows_matching_profile() {
    let p = NodeIdentityPolicy::same_family_default(0x01);
    assert!(p.validate().is_ok());
    assert_eq!(p.permits(0x01), Decision::Allow);
    assert_eq!(p.permits(0x02), Decision::Deny);
}

#[test]
fn open_policy_returns_allow_insecure() {
    let p = NodeIdentityPolicy {
        mode:             TrustMode::Open,
        allowed_profiles: [0; 4],
        allowed_count:    0,
        pinned_roster:    None,
        policy_digest:    Digest32([0u8; 32]),
    };
    assert_eq!(p.permits(0xFF), Decision::AllowInsecure);
    assert_eq!(p.permits(0x00), Decision::AllowInsecure);
}

// IDENTITY:ALLOWED_COUNT_OVERFLOW_REJECTED
#[test]
fn policy_allowed_count_over_capacity_returns_deny() {
    let p = NodeIdentityPolicy {
        mode:             TrustMode::SameFamily,
        allowed_profiles: [0x01, 0x02, 0x03, 0x04],
        allowed_count:    5,  // overflow: array only has 4 slots
        pinned_roster:    None,
        policy_digest:    Digest32([0u8; 32]),
    };
    assert_eq!(p.validate(), Err(PolicyError::AllowedCountOverflow));
    // permits() returns Deny on invalid policy, never panics
    assert_eq!(p.permits(0x01), Decision::Deny);
}

// IDENTITY:FLEET_MODE_REQUIRES_VALID_ROSTER
#[test]
fn fleet_policy_without_roster_is_invalid() {
    let p = NodeIdentityPolicy {
        mode:             TrustMode::Fleet,
        allowed_profiles: [0; 4],
        allowed_count:    0,
        pinned_roster:    None,
        policy_digest:    Digest32([0u8; 32]),
    };
    assert_eq!(p.validate(), Err(PolicyError::FleetWithoutRoster));
    assert_eq!(p.permits(0x01), Decision::Deny);
}

#[test]
fn fleet_policy_with_roster_needs_validation() {
    let p = NodeIdentityPolicy {
        mode:             TrustMode::Fleet,
        allowed_profiles: [0; 4],
        allowed_count:    0,
        pinned_roster:    Some(RosterRef(Digest32([0xAAu8; 32]))),
        policy_digest:    Digest32([0u8; 32]),
    };
    assert!(p.validate().is_ok());
    assert!(matches!(p.permits(0x01), Decision::NeedsRosterValidation(_)));
}

#[test]
fn same_family_empty_allowlist_permits_any() {
    let p = NodeIdentityPolicy {
        mode:             TrustMode::SameFamily,
        allowed_profiles: [0; 4],
        allowed_count:    0,
        pinned_roster:    None,
        policy_digest:    Digest32([0u8; 32]),
    };
    assert_eq!(p.permits(0xFF), Decision::Allow);
    assert_eq!(p.permits(0x00), Decision::Allow);
}

// ── identity_digest function ──────────────────────────────────────────────────

#[test]
fn identity_digest_is_nonzero_for_real_identity() {
    let n = dummy_identity();
    let d = identity_digest(&n);
    assert_ne!(d.0, [0u8; 32]);
}

#[test]
fn identity_digest_deterministic() {
    let n = dummy_identity();
    assert_eq!(identity_digest(&n).0, identity_digest(&n).0);
}

#[test]
fn identity_digest_sensitive_to_node_id() {
    let n1 = dummy_identity();
    let mut n2 = dummy_identity();
    n2.node_id = NodeId([2u8; 16]);
    assert_ne!(identity_digest(&n1).0, identity_digest(&n2).0);
}
