//! Error type for `fjell-keyring` operations.
//!
//! Variants mirror RFC-v0.3-002 §6 and are projected into audit reason
//! codes; renumbering is a breaking change.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u16)]
pub enum SigError {
    /// The chosen algorithm cannot be used in release mode.
    AlgorithmForbiddenInRelease = 0x0001,
    /// The incoming anchor has an epoch <= the current head epoch.
    EpochRegression             = 0x0002,
    /// No anchor for the requested purpose / epoch was found.
    NoAnchorForPurpose          = 0x0003,
    /// The signature did not verify against any active anchor.
    SignatureVerifyFailed       = 0x0004,
    /// The keyring is in `release_mode` but the requested operation would
    /// require `DevDigest32` or other forbidden material.
    ReleaseModeViolation        = 0x0005,
    /// Anchor capacity for this purpose is exhausted.
    AnchorsCapacityExhausted    = 0x0006,
    /// Snapshot magic / schema check failed.
    SnapshotMalformed           = 0x0007,
    /// Snapshot digest check failed.
    SnapshotDigestMismatch      = 0x0008,
    /// Generic internal error.
    Internal                    = 0xFFFF,
}

impl SigError {
    pub const fn code(self) -> u16 {
        self as u16
    }
}
