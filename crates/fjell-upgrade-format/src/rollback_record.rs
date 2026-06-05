//! `RollbackRecord` — append-only anti-rollback state per channel
//! (RFC v0.3-003 §6.2).
//!
//! The *latest* record per `channel_id` in storaged's log is authoritative.
//! The record digest covers all fields so a tampered record is detectable.

use fjell_measure_format::Digest32;

// ── Constants ────────────────────────────────────────────────────────────────

pub const ROLLBACK_RECORD_VERSION: u16 = 1;
pub const ROLLBACK_RECORD_DOMAIN:  &[u8] = b"FJELL-ROLLBACK-V1";

// ── Types ────────────────────────────────────────────────────────────────────

/// Source of a min-counter advance.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum AdvanceSource {
    /// Confirmation by `upgraded` after a positive health probe.
    UpgradedConfirmation = 0x01,
    /// Explicit reset via recovery flow; retains the existing counter.
    RecoveryReset        = 0x02,
    /// bootctl ratchet promotion during boot (defence-in-depth path).
    BootctlPromotion     = 0x03,
}

impl AdvanceSource {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::UpgradedConfirmation),
            0x02 => Some(Self::RecoveryReset),
            0x03 => Some(Self::BootctlPromotion),
            _    => None,
        }
    }
}

/// Persisted anti-rollback floor for a single release channel.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RollbackRecord {
    pub schema_version:      u16,
    /// ASCII release channel identifier (8 B, zero-padded).
    pub channel_id:          [u8; 8],
    /// Current minimum allowed counter for this channel.
    pub min_counter:         u64,
    /// Kernel tick at which this record was written.
    pub last_advance_tick:   u64,
    pub last_advance_source: AdvanceSource,
    /// SHA-256 of all fields above (this field zeroed during computation).
    pub record_digest:       Digest32,
}

impl RollbackRecord {
    /// Compute the canonical `record_digest`.
    pub fn compute_digest(&self) -> Digest32 {
        let sv  = self.schema_version.to_le_bytes();
        let min = self.min_counter.to_le_bytes();
        let tck = self.last_advance_tick.to_le_bytes();
        let src = [self.last_advance_source as u8];
        Digest32::of_parts(&[
            ROLLBACK_RECORD_DOMAIN,
            &sv,
            &self.channel_id,
            &min,
            &tck,
            &src,
            &[0u8; 32], // placeholder for record_digest
        ])
    }

    /// Build a `RollbackRecord` with a freshly computed `record_digest`.
    pub fn new(
        channel_id:     [u8; 8],
        min_counter:    u64,
        advance_tick:   u64,
        source:         AdvanceSource,
    ) -> Self {
        let mut r = Self {
            schema_version:      ROLLBACK_RECORD_VERSION,
            channel_id,
            min_counter,
            last_advance_tick:   advance_tick,
            last_advance_source: source,
            record_digest:       Digest32([0u8; 32]),
        };
        r.record_digest = r.compute_digest();
        r
    }

    /// Return `true` if the recomputed digest matches the stored one.
    pub fn verify_digest(&self) -> bool {
        self.compute_digest() == self.record_digest
    }

    /// A zero-counter genesis record for a fresh channel.
    pub fn genesis(channel_id: [u8; 8]) -> Self {
        Self::new(channel_id, 0, 0, AdvanceSource::UpgradedConfirmation)
    }
}

// ── In-process rollback state tracker ────────────────────────────────────────

/// Anti-rollback check result.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RollbackCheckResult {
    /// `candidate_counter >= min_counter`: allowed to proceed.
    Allowed,
    /// `candidate_counter < min_counter`: must reject.
    Rejected { min_counter: u64 },
    /// `embedded_min_counter > candidate_counter`: metadata is self-contradictory.
    MetadataInconsistent,
}

/// Enforce the rollback policy against a candidate release counter.
///
/// Returns the new `min_counter` to persist on success; caller is responsible
/// for the storaged append.
pub fn check_rollback(
    persisted_min:         u64,
    candidate_counter:     u64,
    embedded_min_counter:  u64,
) -> RollbackCheckResult {
    // Self-consistency: author floor must not exceed the counter.
    if embedded_min_counter > candidate_counter {
        return RollbackCheckResult::MetadataInconsistent;
    }
    // Anti-rollback: reject anything below the persisted floor.
    if candidate_counter < persisted_min {
        return RollbackCheckResult::Rejected { min_counter: persisted_min };
    }
    RollbackCheckResult::Allowed
}

/// Compute the new `min_counter` to persist after a successful confirmation.
pub fn advance_min_counter(persisted_min: u64, confirmed_counter: u64) -> u64 {
    persisted_min.max(confirmed_counter)
}
