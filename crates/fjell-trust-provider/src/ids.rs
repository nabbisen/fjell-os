//! Stable identifiers for trust providers and key purposes.
//!
//! These types are part of the v0.3 stable surface and must never be made
//! larger than `u32` to keep the IPC encoding cheap.

/// Stable identifier for a registered trust provider.
///
/// The value `0` is reserved as "unset" and never names a real provider.
/// Identifiers are assigned by the `ProviderRegistry` in the order providers
/// are registered, so the actual values depend on the boot profile.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct TrustProviderId(pub u32);

impl TrustProviderId {
    /// Sentinel "unset" / "unknown provider" value.  Must never name a real
    /// provider.
    pub const UNSET: Self = Self(0);

    /// Construct an id.  `id == 0` is normalised to `UNSET`.
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// True if this is the sentinel value.
    pub const fn is_unset(self) -> bool {
        self.0 == 0
    }
}

/// User-space handle for a registered trust provider.
///
/// The handle is a generation-tagged pair (`id`, `generation`).  Registries
/// rotate `generation` on every replace/remove so a stale `ProviderHandle`
/// always fails the use-time check, even if the slot number is later reused.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ProviderHandle {
    pub id: TrustProviderId,
    pub generation: u16,
}

impl ProviderHandle {
    pub const UNSET: Self = Self {
        id: TrustProviderId::UNSET,
        generation: 0,
    };

    pub const fn new(id: TrustProviderId, generation: u16) -> Self {
        Self { id, generation }
    }

    pub const fn is_unset(self) -> bool {
        self.id.is_unset()
    }
}

/// Purpose of a key being sealed or used by a trust provider.
///
/// The enum is intentionally narrow: every purpose corresponds to a security
/// boundary that already exists in the v0.2 design (release verification,
/// rootfs verification, policy verification, snapshot signing, attestation
/// signing).  Adding a new purpose is a security-boundary change and requires
/// an ADR.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum KeyPurpose {
    /// Verify release manifests (signed bundles produced by upgraded).
    ReleaseVerification = 0x01,
    /// Verify rootfs manifests.
    RootfsVerification = 0x02,
    /// Verify policy bundles (cap-broker policy distribution).
    PolicyVerification = 0x03,
    /// Sign a local attestation record.
    AttestationSigning = 0x04,
    /// Seal/unseal a derived key (e.g. per-boot data key).
    SealedDataKey = 0x05,
    /// Sign exported snapshots (v0.7 forward-compat reservation).
    SnapshotSigning = 0x06,
}

impl KeyPurpose {
    /// Stable byte tag for use in canonical digests / on-wire records.
    pub const fn tag(self) -> u8 {
        self as u8
    }

    /// All purposes that are valid in v0.3.0.  v0.7 adds snapshot signing.
    pub const fn all() -> &'static [KeyPurpose] {
        &[
            Self::ReleaseVerification,
            Self::RootfsVerification,
            Self::PolicyVerification,
            Self::AttestationSigning,
            Self::SealedDataKey,
            Self::SnapshotSigning,
        ]
    }

    /// True if this purpose verifies an external bundle (no signing power).
    pub const fn is_verification_only(self) -> bool {
        matches!(
            self,
            Self::ReleaseVerification | Self::RootfsVerification | Self::PolicyVerification
        )
    }
}
