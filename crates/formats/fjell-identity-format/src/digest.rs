//! Canonical SHA-256 identity digest (RFC v0.7-001 §6.1).

use fjell_measure_format::Digest32;
use crate::identity::{NodeIdentity, NODE_IDENTITY_SCHEMA_VERSION};

/// Compute the canonical `identity_digest` for a `NodeIdentity`.
///
/// Wire layout per RFC v0.7-001 §6.1:
/// ```text
/// SHA256("FJELL-NODE-ID-V1" ||
///        schema u16 LE     ||
///        node_id 16 B      ||
///        alias 32 B        ||
///        created_tick u64 LE       ||
///        trust_provider_id u32 LE  ||
///        trust_profile_tag u8      ||
///        attestation_pubkey 32 B   ||
///        platform_digest 32 B      ||
///        board_digest 32 B)
/// ```
pub fn identity_digest(n: &NodeIdentity) -> Digest32 {
    let mut buf = [0u8; 256];
    let mut pos = 0usize;

    macro_rules! w_u8  { ($v:expr) => { buf[pos] = $v; pos += 1; }; }
    macro_rules! w_u16 { ($v:expr) => { buf[pos..pos+2].copy_from_slice(&($v as u16).to_le_bytes()); pos += 2; }; }
    macro_rules! w_u32 { ($v:expr) => { buf[pos..pos+4].copy_from_slice(&($v as u32).to_le_bytes()); pos += 4; }; }
    macro_rules! w_u64 { ($v:expr) => { buf[pos..pos+8].copy_from_slice(&($v as u64).to_le_bytes()); pos += 8; }; }
    macro_rules! w_bytes { ($b:expr) => { let bb: &[u8] = $b; buf[pos..pos+bb.len()].copy_from_slice(bb); pos += bb.len(); }; }

    w_bytes!(b"FJELL-NODE-ID-V1");
    w_u16!(NODE_IDENTITY_SCHEMA_VERSION);
    w_bytes!(&n.node_id.0);
    w_bytes!(&n.alias.0);
    w_u64!(n.created_tick);
    w_u32!(n.trust_provider_id);
    w_u8! (n.trust_profile_tag);
    w_bytes!(&n.attestation_pubkey.0);
    w_bytes!(&n.platform_digest.0);
    w_bytes!(&n.board_digest.0);

    Digest32::of(&buf[..pos])
}
