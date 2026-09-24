//! Canonical SHA-256 identity digest (RFC v0.7-001 §6.1).

use crate::identity::{NODE_IDENTITY_SCHEMA_VERSION, NodeIdentity};
use fjell_canon::{BufSink, Canon};
use fjell_measure_format::Digest32;

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
    let mut sink = BufSink::<256>::new();
    write_canonical(n, &mut sink);
    Digest32::of(sink.bytes())
}

/// The canonical byte stream `identity_digest` is taken over — **the function
/// the digest is computed from and the frozen schema is generated from**.
pub fn write_canonical(n: &NodeIdentity, c: &mut dyn Canon) {
    c.domain(b"FJELL-NODE-ID-V1");
    c.u16("schema_version", NODE_IDENTITY_SCHEMA_VERSION);
    c.bytes("node_id", &n.node_id.0);
    c.bytes("alias", &n.alias.0);
    c.u64("created_tick", n.created_tick);
    c.u32("trust_provider_id", n.trust_provider_id);
    c.u8("trust_profile_tag", n.trust_profile_tag);
    c.bytes("attestation_pubkey", &n.attestation_pubkey.0);
    c.bytes("platform_digest", &n.platform_digest.0);
    c.bytes("board_digest", &n.board_digest.0);
}
