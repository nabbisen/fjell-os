//! Provider kind, capability flags, run-time state, and trust profile.

/// Kind tag for the trust provider implementation.
///
/// `Null` providers are explicitly **forbidden** in the release profile and
/// must be rejected at registry-enforcing time.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum TrustProviderKind {
    /// Software-only provider used in QEMU/local tests.
    Development = 0x01,
    /// Future TPM-backed provider (placeholder kind, no impl in v0.3.0-alpha).
    Tpm = 0x02,
    /// Future DICE-backed provider (placeholder kind).
    Dice = 0x03,
    /// Test-only provider that always fails.  Never permitted in a release
    /// profile registry.
    Null = 0x04,
}

impl TrustProviderKind {
    pub const fn tag(self) -> u8 {
        self as u8
    }

    /// True if this provider kind is permitted in a release-profile registry.
    ///
    /// `Null` is never permitted.  `Development` is permitted in v0.3.0-alpha
    /// because the project is explicitly pre-production; once a hardware
    /// profile lands this should be tightened by an ADR.
    pub const fn permitted_in_release(self) -> bool {
        !matches!(self, Self::Null)
    }
}

/// Bit-flags describing which operations the provider can perform.
///
/// A provider is allowed to expose a subset of operations.  Callers must
/// consult `TrustProviderDescriptor::capabilities` before invoking, and the
/// provider trait's default impls also return `TrustError::NotSupported` for
/// missing capabilities.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TrustProviderCapabilities(pub u32);

impl TrustProviderCapabilities {
    pub const NONE: Self = Self(0);
    pub const READ_MEASUREMENT: Self = Self(1 << 0);
    pub const SIGN_ATTESTATION: Self = Self(1 << 1);
    pub const READ_ROLLBACK_COUNTER: Self = Self(1 << 2);
    pub const SEAL_KEY: Self = Self(1 << 3);
    pub const UNSEAL_KEY: Self = Self(1 << 4);

    /// All capabilities a development provider must expose.
    pub const DEVELOPMENT_BASELINE: Self = Self(
        Self::READ_MEASUREMENT.0
            | Self::SIGN_ATTESTATION.0
            | Self::READ_ROLLBACK_COUNTER.0
            | Self::SEAL_KEY.0
            | Self::UNSEAL_KEY.0,
    );

    pub const fn empty() -> Self {
        Self::NONE
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// Run-time state of a registered provider.
///
/// `Bootstrap` is the initial state when the provider is registered but not
/// yet authoritative; `Active` is the normal state; `Faulted` is set when the
/// provider observes an unrecoverable internal error (which must be reported
/// to the semantic stream and audit).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum TrustProviderState {
    Bootstrap = 0x01,
    Active = 0x02,
    Faulted = 0x03,
    Withdrawn = 0x04,
}

impl TrustProviderState {
    pub const fn tag(self) -> u8 {
        self as u8
    }

    pub const fn is_usable(self) -> bool {
        matches!(self, Self::Bootstrap | Self::Active)
    }
}

/// Stable identifier for a trust profile.
///
/// The trust profile describes *which evidence shape* the provider follows.
/// `FjellLocalV1` is the local development profile.  Adding a new profile is
/// a security-boundary change and must be tracked in an ADR.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum TrustProfile {
    FjellLocalV1 = 0x01,
    /// Reserved for v0.3.x — TPM 2.0 attestation profile.
    FjellTpmV1 = 0x02,
    /// Reserved for v0.3.x — DICE profile for embedded boards.
    FjellDiceV1 = 0x03,
}

impl TrustProfile {
    pub const fn tag(self) -> u8 {
        self as u8
    }
}
