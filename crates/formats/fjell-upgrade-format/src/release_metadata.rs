//! `ReleaseMetadata` — per-release record binding a manifest digest to a
//! monotonic release counter, keyring epoch, trust provider, and measurement
//! chain head (RFC v0.3-003 §6.1).

use fjell_measure_format::{Digest32, MeasurementHead};
use fjell_keyring::KeyEpoch;
use fjell_trust_provider::ids::TrustProviderId;

// ── Constants ────────────────────────────────────────────────────────────────

pub const RELEASE_METADATA_VERSION: u16 = 1;
pub const RELEASE_METADATA_DOMAIN: &[u8] = b"FJELL-RELEASE-META-V1";

// ── Types ────────────────────────────────────────────────────────────────────

/// Provenance info embedded by the release builder.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Provenance {
    /// Tool identifier e.g. `b"fjell-bu"` (8 B ASCII, zero-padded).
    pub builder_tool_id: [u8; 8],
    /// Tool version string e.g. `b"0.3.0a1\0"`.
    pub builder_version: [u8; 8],
}

impl Provenance {
    pub const DEV: Self = Self {
        builder_tool_id: *b"dev-tool",
        builder_version: *b"0.3.0\0\0\0",
    };
}

/// Per-release record that binds a manifest digest to a monotonic counter and
/// all trust-relevant context captured at staging time.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ReleaseMetadata {
    pub schema_version:          u16,
    /// ASCII release channel identifier e.g. `b"stable\0\0"`.
    pub channel_id:              [u8; 8],
    /// Monotonically increasing counter within this channel.
    pub release_counter:         u64,
    /// Author-specified minimum counter that receivers must already hold.
    pub embedded_min_counter:    u64,
    /// SHA-256 of the release manifest.
    pub release_manifest_digest: Digest32,
    /// `KeyEpoch` of the anchor that signed this release.
    pub signing_anchor_epoch:    KeyEpoch,
    /// Trust provider ID that authorised staging.
    pub trust_provider_id:       TrustProviderId,
    /// `MeasurementHead.chain_digest` at staging time.
    pub measurement_at_stage:    Digest32,
    /// Local kernel tick at staging time.
    pub created_tick:            u64,
    pub provenance:              Provenance,
    /// SHA-256 of all fields above (this field zeroed during computation).
    pub metadata_digest:         Digest32,
}

impl ReleaseMetadata {
    /// Compute the canonical `metadata_digest`.
    /// `metadata_digest` itself is treated as `[0u8; 32]` during hashing.
    pub fn compute_digest(&self) -> Digest32 {
        let sv  = self.schema_version.to_le_bytes();
        let ctr = self.release_counter.to_le_bytes();
        let min = self.embedded_min_counter.to_le_bytes();
        let aep = self.signing_anchor_epoch.raw().to_le_bytes();
        let pid = self.trust_provider_id.0.to_le_bytes();
        let tck = self.created_tick.to_le_bytes();
        Digest32::of_parts(&[
            RELEASE_METADATA_DOMAIN,
            &sv,
            &self.channel_id,
            &ctr,
            &min,
            &self.release_manifest_digest.0,
            &aep,
            &pid,
            &self.measurement_at_stage.0,
            &tck,
            &self.provenance.builder_tool_id,
            &self.provenance.builder_version,
            &[0u8; 32], // placeholder for metadata_digest
        ])
    }

    /// Build a `ReleaseMetadata` with a freshly computed `metadata_digest`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        channel_id:              [u8; 8],
        release_counter:         u64,
        embedded_min_counter:    u64,
        release_manifest_digest: Digest32,
        signing_anchor_epoch:    KeyEpoch,
        trust_provider_id:       TrustProviderId,
        measurement_at_stage:    Digest32,
        created_tick:            u64,
        provenance:              Provenance,
    ) -> Self {
        let mut m = Self {
            schema_version: RELEASE_METADATA_VERSION,
            channel_id,
            release_counter,
            embedded_min_counter,
            release_manifest_digest,
            signing_anchor_epoch,
            trust_provider_id,
            measurement_at_stage,
            created_tick,
            provenance,
            metadata_digest: Digest32([0u8; 32]),
        };
        m.metadata_digest = m.compute_digest();
        m
    }

    /// Return `true` if the recomputed digest matches the stored one.
    pub fn verify_digest(&self) -> bool {
        self.compute_digest() == self.metadata_digest
    }

    /// Return `true` if `embedded_min_counter <= release_counter`.
    /// A self-contradictory bundle is rejected before checking signatures.
    pub fn is_internally_consistent(&self) -> bool {
        self.embedded_min_counter <= self.release_counter
    }

    /// Development fixture with known-good counter and digest.
    pub fn dev(channel_id: [u8; 8], counter: u64) -> Self {
        Self::new(
            channel_id,
            counter,
            counter.saturating_sub(1),
            Digest32([0xAA; 32]),
            KeyEpoch::ONE,
            TrustProviderId::new(0x01),
            Digest32([0xBB; 32]),
            0,
            Provenance::DEV,
        )
    }
}

/// Extract the chain-digest from a `MeasurementHead` for embedding in metadata.
pub fn chain_digest_from_head(head: MeasurementHead) -> Digest32 {
    head.chain_digest
}
