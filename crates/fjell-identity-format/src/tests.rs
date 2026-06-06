//! Host unit tests for `fjell-identity-format` (RFC v0.7-001 §11).

use crate::identity::{
    NodeIdentity, NodeId, NodeAlias, AttestationPubkey,
    NODE_IDENTITY_SCHEMA_VERSION, STORE_RECORD_KIND_IDENTITY,
};
use crate::policy::{NodeIdentityPolicy, TrustMode};
use crate::digest::identity_digest;
use fjell_measure_format::Digest32;

fn dummy_identity() -> NodeIdentity {
    NodeIdentity::new(
        NodeId([1u8; 16]),
        NodeAlias(*b"test-node-00\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0"),
        1_000_000,
        42,
        0x01,
        AttestationPubkey([0xABu8; 32]),
        Digest32([0x11u8; 32]),
        Digest32([0x22u8; 32]),
    )
}

// ── Schema constants ──────────────────────────────────────────────────────────

#[test]
fn schema_version_is_one() {
    assert_eq!(NODE_IDENTITY_SCHEMA_VERSION, 1);
}

#[test]
fn store_record_kind_identity_value() {
    assert_eq!(STORE_RECORD_KIND_IDENTITY, 0x0020);
}

// ── NodeAlias ─────────────────────────────────────────────────────────────────

#[test]
fn node_alias_as_str_trims_at_nul() {
    let mut a = NodeAlias([0u8; 32]);
    a.0[..9].copy_from_slice(b"my-node-1");
    assert_eq!(a.as_str(), "my-node-1");
}

#[test]
fn node_alias_all_nul_is_empty() {
    assert_eq!(NodeAlias([0u8; 32]).as_str(), "");
}

// ── identity_digest ───────────────────────────────────────────────────────────

#[test]
fn identity_digest_is_nonzero() {
    let n = dummy_identity();
    let d = identity_digest(&n);
    assert_ne!(d.0, [0u8; 32]);
}

#[test]
fn identity_digest_is_deterministic() {
    let n = dummy_identity();
    assert_eq!(identity_digest(&n).0, identity_digest(&n).0);
}

#[test]
fn identity_digest_changes_with_node_id() {
    let n1 = dummy_identity();
    let mut n2 = dummy_identity();
    n2.node_id = NodeId([2u8; 16]);
    let d1 = identity_digest(&n1);
    let d2 = identity_digest(&n2);
    assert_ne!(d1.0, d2.0);
}

#[test]
fn identity_digest_changes_with_platform_digest() {
    let n1 = dummy_identity();
    let mut n2 = dummy_identity();
    n2.platform_digest = Digest32([0x99u8; 32]);
    assert_ne!(identity_digest(&n1).0, identity_digest(&n2).0);
}

// ── NodeIdentityPolicy ────────────────────────────────────────────────────────

#[test]
fn same_family_policy_permits_matching_profile() {
    let p = NodeIdentityPolicy::same_family_default(0x01);
    assert!(p.permits(0x01));
    assert!(!p.permits(0x02));
}

#[test]
fn open_policy_permits_any_profile() {
    let mut p = NodeIdentityPolicy::same_family_default(0x01);
    p.mode = TrustMode::Open;
    assert!(p.permits(0xFF));
    assert!(p.permits(0x00));
}

#[test]
fn trust_mode_roundtrip() {
    for (byte, expected) in [(1u8, TrustMode::SameFamily), (2, TrustMode::Fleet), (3, TrustMode::Open)] {
        assert_eq!(TrustMode::from_u8(byte).unwrap() as u8, expected as u8);
    }
    assert!(TrustMode::from_u8(0).is_none());
}
