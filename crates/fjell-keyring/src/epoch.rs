//! Monotonic key epoch.
//!
//! Every anchor carries a `KeyEpoch`.  When a purpose is rotated, the
//! incoming anchor must have an epoch strictly greater than the highest
//! epoch currently active for that purpose.  Lower epochs are rejected at
//! install time with `SigError::EpochRegression`.

/// A monotonically-increasing key epoch for one `KeyPurpose`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct KeyEpoch(pub u32);

impl KeyEpoch {
    /// Conventional "no epoch" sentinel.  Not assignable to a live anchor.
    pub const ZERO: Self = Self(0);

    /// Initial real epoch for a freshly-installed genesis anchor.
    pub const ONE: Self = Self(1);

    pub const fn raw(self) -> u32 {
        self.0
    }
}
