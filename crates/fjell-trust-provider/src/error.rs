//! Error type for `HardwareTrustProvider` operations.
//!
//! Every variant is mapped 1:1 to an audit-visible reason code so that
//! security-relevant failures are observable in the semantic stream and
//! audit log.

/// All failures observable from a `HardwareTrustProvider` operation.
///
/// The enum is `#[repr(u16)]` because the values are projected into audit
/// records and the semantic stream.  Changing or reordering variants is a
/// breaking change and requires an ADR.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u16)]
pub enum TrustError {
    /// The provider does not implement this operation.
    NotSupported = 0x0001,
    /// The provider is in `Faulted` or `Withdrawn` state.
    ProviderUnavailable = 0x0002,
    /// A `Null` (test-only) provider was selected in a release-mode registry.
    NullProviderForbidden = 0x0003,
    /// The provided `ProviderHandle` has a stale generation.
    StaleHandle = 0x0004,
    /// The caller asked to unseal a key for a different purpose than the one
    /// it was sealed for.
    PurposeMismatch = 0x0005,
    /// The sealed blob failed an integrity check.
    SealIntegrityFailed = 0x0006,
    /// The anti-rollback counter cannot advance (provider-internal limit).
    RollbackCounterExhausted = 0x0007,
    /// Key material exceeded the maximum supported length.
    KeyMaterialTooLarge = 0x0008,
    /// Signature could not be produced (internal hash/signing failure).
    SignFailed = 0x0009,
    /// Generic internal error — last resort.
    Internal = 0xFFFF,
}

impl TrustError {
    pub const fn code(self) -> u16 {
        self as u16
    }
}
