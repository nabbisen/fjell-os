//! `NodeIdentity` wire type (RFC v0.7-001 §6.1, §6.3).
//!
//! v0.7.1 note: `NodeIdentity::new()` is preserved for backward-compat but
//! produces a zero `identity_digest`. Prefer `NodeIdentity::build()` which
//! computes the digest at construction time (RFC-v0.7.2-003).

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
    /// Strict UTF-8 access — returns Err on invalid sequences (RFC-v0.7.2-003).
    /// Use this in security-sensitive paths (logging, policy checks).
    pub fn try_as_str(&self) -> Result<&str, core::str::Utf8Error> {
        let end = self.0.iter().position(|&b| b == 0).unwrap_or(32);
        core::str::from_utf8(&self.0[..end])
    }

    /// Lossy display helper — replaces invalid UTF-8 with a static marker.
    /// Use ONLY for human-readable diagnostic output; never in policy paths.
    pub fn as_str_lossy(&self) -> &str {
        self.try_as_str().unwrap_or("<invalid-utf8-alias>")
    }
}

/// Ed25519 public key used for attestation signing (32 bytes).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct AttestationPubkey(pub [u8; 32]);

/// Typed identity construction / validation error (RFC-v0.7.2-003).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum IdentityError {
    /// `identity_digest` was all-zero after computation (should not happen).
    DigestComputationFailed = 0x01,
    /// Stored `identity_digest` does not match recomputed value (corruption).
    DigestMismatch          = 0x02,
    /// Alias contains invalid UTF-8.
    InvalidAlias            = 0x03,
}

/// Builder for a `NodeIdentity` (RFC-v0.7.2-003).
pub struct NodeIdentityBuilder {
    pub node_id:            NodeId,
    pub alias:              NodeAlias,
    pub created_tick:       u64,
    pub trust_provider_id:  u32,
    pub trust_profile_tag:  u8,
    pub attestation_pubkey: AttestationPubkey,
    pub platform_digest:    Digest32,
    pub board_digest:       Digest32,
}

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
    /// Safe constructor — computes `identity_digest` at build time and
    /// returns `Err` if the digest is all-zero (RFC-v0.7.2-003, closes C-H-04).
    ///
    /// This is the PREFERRED path. Use `new()` only for backward compatibility.
    pub fn build(b: NodeIdentityBuilder) -> Result<Self, IdentityError> {
        let mut n = Self {
            schema_version:     NODE_IDENTITY_SCHEMA_VERSION,
            node_id:            b.node_id,
            alias:              b.alias,
            created_tick:       b.created_tick,
            trust_provider_id:  b.trust_provider_id,
            trust_profile_tag:  b.trust_profile_tag,
            attestation_pubkey: b.attestation_pubkey,
            platform_digest:    b.platform_digest,
            board_digest:       b.board_digest,
            identity_digest:    Digest32([0u8; 32]),
        };
        n.identity_digest = crate::digest::identity_digest(&n);
        if n.identity_digest.0 == [0u8; 32] {
            return Err(IdentityError::DigestComputationFailed);
        }
        Ok(n)
    }

    /// Validate that the stored `identity_digest` matches a freshly recomputed
    /// one. Returns `Err(DigestMismatch)` on failure.
    pub fn validate_digest(&self) -> Result<(), IdentityError> {
        let computed = crate::digest::identity_digest(self);
        if computed.0 != self.identity_digest.0 {
            Err(IdentityError::DigestMismatch)
        } else {
            Ok(())
        }
    }

    /// Legacy constructor — preserves backward compatibility but produces a
    /// zero `identity_digest`. Prefer `NodeIdentity::build()` for new code.
    ///
    /// Callers that use this method MUST call `identity_digest()` and write
    /// back before storing, or call `validate_digest()` on load.
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
