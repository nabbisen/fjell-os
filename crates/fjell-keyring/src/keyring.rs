//! `Keyring` — fixed-size, per-`KeyPurpose` anchor container.
//!
//! Anchors are stored in a flat array, grouped by `KeyPurpose`.  The
//! container is alloc-free and `Copy`-friendly for the parts that
//! cross IPC boundaries (descriptors), while the full `Keyring` itself
//! is large enough to live behind an owning service.
//!
//! See RFC-v0.3-002 §6 for the data model.

use crate::anchor::TrustAnchor;
use crate::epoch::KeyEpoch;
use crate::error::SigError;
use crate::ANCHORS_PER_PURPOSE;
use fjell_trust_provider::KeyPurpose;

/// Number of key purposes we slot in v0.3.0.  This must equal the number
/// of `KeyPurpose` variants returned by `KeyPurpose::all()`.
pub const PURPOSE_SLOT_COUNT: usize = 7;  // v0.5.0: +BoardProfile

/// Fixed-size keyring container.
pub struct Keyring {
    anchors: [[Option<TrustAnchor>; ANCHORS_PER_PURPOSE]; PURPOSE_SLOT_COUNT],
    release_mode: bool,
}

impl Keyring {
    pub const fn new() -> Self {
        Self {
            anchors: [[None; ANCHORS_PER_PURPOSE]; PURPOSE_SLOT_COUNT],
            release_mode: false,
        }
    }

    /// Returns whether the keyring rejects development algorithms.
    pub const fn release_mode(&self) -> bool {
        self.release_mode
    }

    /// One-way enable of release-mode.  After this returns, attempts to
    /// install or look up an anchor whose algorithm is
    /// `SignatureAlgorithm::DevDigest32` will be rejected.
    pub fn enter_release_mode(&mut self) {
        self.release_mode = true;
    }

    /// Install an anchor for its purpose.
    ///
    /// Rules:
    ///   - new anchor's `epoch` must be strictly greater than every
    ///     existing anchor for the same purpose;
    ///   - in `release_mode`, `DevDigest32` algorithms are forbidden;
    ///   - if all slots for the purpose are full, the oldest (lowest
    ///     epoch) is evicted to make room for the new anchor.
    pub fn install(&mut self, anchor: TrustAnchor) -> Result<(), SigError> {
        if self.release_mode && !anchor.algorithm.permitted_in_release() {
            return Err(SigError::ReleaseModeViolation);
        }
        let slot = purpose_index(anchor.purpose);

        // Epoch must be strictly greater than the current max for this purpose.
        let max_epoch = self.anchors[slot]
            .iter()
            .flatten()
            .map(|a| a.epoch.raw())
            .max()
            .unwrap_or(0);
        if anchor.epoch.raw() <= max_epoch {
            return Err(SigError::EpochRegression);
        }

        // Find a free slot, or evict the lowest-epoch anchor.
        if let Some(free) = self.anchors[slot].iter_mut().find(|s| s.is_none()) {
            *free = Some(anchor);
            return Ok(());
        }
        // No free slot: evict the lowest-epoch entry.
        let evict_idx = self.anchors[slot]
            .iter()
            .enumerate()
            .min_by_key(|(_, a)| a.unwrap().epoch.raw())
            .map(|(i, _)| i)
            .ok_or(SigError::AnchorsCapacityExhausted)?;
        self.anchors[slot][evict_idx] = Some(anchor);
        Ok(())
    }

    /// Return the highest active epoch for `purpose`, or `None` if no
    /// anchors are installed.
    pub fn active_epoch(&self, purpose: KeyPurpose) -> Option<KeyEpoch> {
        let slot = purpose_index(purpose);
        self.anchors[slot]
            .iter()
            .flatten()
            .map(|a| a.epoch)
            .max()
    }

    /// Look up the anchor with the highest epoch for `purpose`.
    pub fn latest(&self, purpose: KeyPurpose) -> Option<TrustAnchor> {
        let slot = purpose_index(purpose);
        self.anchors[slot]
            .iter()
            .flatten()
            .copied()
            .max_by_key(|a| a.epoch.raw())
    }

    /// Iterate all anchors registered for `purpose` (in unspecified order).
    pub fn anchors_for(&self, purpose: KeyPurpose) -> impl Iterator<Item = TrustAnchor> + '_ {
        let slot = purpose_index(purpose);
        self.anchors[slot].iter().flatten().copied()
    }

    /// Total number of live anchors across all purposes.
    pub fn len(&self) -> usize {
        self.anchors
            .iter()
            .map(|row| row.iter().flatten().count())
            .sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for Keyring {
    fn default() -> Self {
        Self::new()
    }
}

/// Map a `KeyPurpose` to an internal slot index.
const fn purpose_index(purpose: KeyPurpose) -> usize {
    match purpose {
        KeyPurpose::ReleaseVerification => 0,
        KeyPurpose::RootfsVerification => 1,
        KeyPurpose::PolicyVerification => 2,
        KeyPurpose::AttestationSigning => 3,
        KeyPurpose::SealedDataKey => 4,
        KeyPurpose::SnapshotSigning => 5,
        KeyPurpose::BoardProfile => 6,
    }
}
