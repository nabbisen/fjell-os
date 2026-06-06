//! `NodeIdentity` wire type (RFC v0.7-001 §6.1, §6.3).

use fjell_measure_format::Digest32;

pub const NODE_IDENTITY_SCHEMA_VERSION: u16 = 1;
/// `StoreRecordKind` tag for a committed `NodeIdentity` record (v0.7-001 §6.3).
pub const STORE_RECORD_KIND_IDENTITY: u16 = 0x0020;

/// 16-byte opaque node identifier (random at first boot, persisted).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct NodeId(pub [u8; 16]);

/// 32-byte human-readable alias (UTF-8, zero-padded).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct NodeAlias(pub [u8; 32]);

impl NodeAlias {
    /// Return the alias as a `str` slice (up to the first NUL or end).
    pub fn as_str(&self) -> &str {
        let end = self.0.iter().position(|&b| b == 0).unwrap_or(32);
        core::str::from_utf8(&self.0[..end]).unwrap_or("")
    }
}

/// Ed25519 public key used for attestation signing (32 bytes).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct AttestationPubkey(pub [u8; 32]);

/// Full node identity record.
///
/// `identity_digest` is computed by `crate::digest::identity_digest` over
/// the canonical prefix; it must be verified on load.
#[derive(Clone, Copy, Debug)]
pub struct NodeIdentity {
    pub schema_version:     u16,
    pub node_id:            NodeId,
    pub alias:              NodeAlias,
    pub created_tick:       u64,
    pub trust_provider_id:  u32,
    pub trust_profile_tag:  u8,
    pub attestation_pubkey: AttestationPubkey,
    /// SHA-256 of the `PlatformProfile` (from `fjell-platform-format`).
    pub platform_digest:    Digest32,
    /// SHA-256 of the `BoardProfile`.
    pub board_digest:       Digest32,
    /// Canonical digest over all the above fields.
    pub identity_digest:    Digest32,
}

impl NodeIdentity {
    /// Construct a skeleton identity (digests zeroed; call `identity_digest`
    /// and write back before storing).
    pub fn new(
        node_id:            NodeId,
        alias:              NodeAlias,
        created_tick:       u64,
        trust_provider_id:  u32,
        trust_profile_tag:  u8,
        attestation_pubkey: AttestationPubkey,
        platform_digest:    Digest32,
        board_digest:       Digest32,
    ) -> Self {
        Self {
            schema_version:     NODE_IDENTITY_SCHEMA_VERSION,
            node_id,
            alias,
            created_tick,
            trust_provider_id,
            trust_profile_tag,
            attestation_pubkey,
            platform_digest,
            board_digest,
            identity_digest:    Digest32([0u8; 32]),
        }
    }
}
