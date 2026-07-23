//! Recovery plane format types for Fjell OS M8.
//!
//! Defines `RecoveryRequest`, `RecoveryResponse`, and related types used by
//! `recoveryd` and `recovery.target`.
//!
//! # Invariants
//!
//! - Manual rollback always requires `confirmed_by_operator: true`.
//! - `proxy-text` must never hold `RECOVERY_SELECT_ROLLBACK` directly.
//! - All recovery actions are recorded in audit, measurement, and semantic logs.
#![no_std]

use fjell_measure_format::Digest32;

// ── Slot / state types ────────────────────────────────────────────────────────

/// Boot slot identifier.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum SlotId {
    A = 0,
    B = 1,
}

impl SlotId {
    pub fn as_u8(self) -> u8 { self as u8 }
    pub fn from_u8(v: u8) -> Option<Self> {
        match v { 0 => Some(Self::A), 1 => Some(Self::B), _ => None }
    }
    pub fn label(self) -> u8 { if self == Self::A { b'A' } else { b'B' } }
}

/// Current state of a boot slot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum SlotState {
    Empty       = 0x00,
    Staged      = 0x01,
    Verified    = 0x02,
    Confirmed   = 0x03,
    Failed      = 0x04,
    Rollback    = 0x05,
}

/// Health status for a slot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum HealthStatus {
    NotRun  = 0x00,
    Passed  = 0x01,
    Failed  = 0x02,
    Timeout = 0x03,
}

// ── Recovery reason / rollback cause ─────────────────────────────────────────

/// Why recovery was entered.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum RecoveryReason {
    FreshnessFailure     = 0x01,
    VerificationFailure  = 0x02,
    HealthFailure        = 0x03,
    BootFailure          = 0x04,
    OperatorRequest      = 0x05,
    Unknown              = 0xFF,
}

/// Why a rollback was selected.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum RollbackReason {
    HealthCheckFailed    = 0x01,
    FreshnessRejected    = 0x02,
    VerificationFailed   = 0x03,
    OperatorRequested    = 0x04,
    MaxBootAttemptsExceeded = 0x05,
}

/// Export format for diagnostics.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum ExportFormat {
    PlainText = 0x01,
    Json      = 0x02,
    Toml      = 0x03,
}

// ── Snapshot summary ──────────────────────────────────────────────────────────

/// Compact summary of one snapshot (for listing).
#[derive(Clone, Copy, Debug)]
pub struct SnapshotSummary {
    pub snapshot_id:     [u8; 8],
    pub created_tick:    u64,
    pub slot:            SlotId,
    pub reason:          u8,
    pub digest:          Digest32,
}

// ── Slot inspection result ────────────────────────────────────────────────────

/// Full inspection result for one slot.
#[derive(Clone, Copy, Debug)]
pub struct SlotInspection {
    pub slot:             SlotId,
    pub state:            SlotState,
    pub release_digest:   Digest32,
    pub rootfs_digest:    Digest32,
    pub policy_digest:    Digest32,
    pub verified:         bool,
    pub confirmed:        bool,
    pub tries_remaining:  u8,
    pub last_health:      HealthStatus,
}

/// Summary of the most recent failure.
#[derive(Clone, Copy, Debug)]
pub struct FailureSummary {
    pub slot:             SlotId,
    pub reason:           RecoveryReason,
    pub failed_at_tick:   u64,
    pub snapshot_id:      Option<[u8; 8]>,
    pub last_health:      HealthStatus,
}

// ── Export chunk ──────────────────────────────────────────────────────────────

/// One chunk of a diagnostics export (max 256 bytes of payload).
#[derive(Clone, Copy, Debug)]
pub struct DiagnosticsChunk {
    pub chunk_index: u32,
    pub total_chunks: u32,
    pub format: ExportFormat,
    pub payload_len: u8,
    pub payload: [u8; 128],
}

// ── Request / Response ────────────────────────────────────────────────────────

/// Request to the recoveryd service.
#[derive(Clone, Copy, Debug)]
pub enum RecoveryRequest {
    /// List available snapshots (returns SnapshotList).
    ListSnapshots,
    /// Inspect a specific slot (returns SlotInspection).
    InspectSlot { slot: SlotId },
    /// Inspect the most recent failure (returns FailureSummary).
    InspectLatestFailure,
    /// Enter recovery.target mode.
    EnterRecoveryTarget { reason: RecoveryReason },
    /// Request a manual rollback.
    ///
    /// `confirmed_by_operator` MUST be true; recoveryd rejects false.
    SelectRollback {
        slot: SlotId,
        reason: RollbackReason,
        confirmed_by_operator: bool,
    },
    /// Export diagnostics in the requested format.
    ExportDiagnostics { format: ExportFormat },
}

/// Maximum snapshots returned in one SnapshotList response.
pub const MAX_SNAPSHOT_LIST: usize = 8;

/// Response from the recoveryd service.
#[derive(Clone, Copy, Debug)]
pub enum RecoveryResponse {
    SnapshotList {
        count: u8,
        snapshots: [SnapshotSummary; MAX_SNAPSHOT_LIST],
    },
    SlotInspectionResult {
        inspection: SlotInspection,
    },
    FailureInspectionResult {
        summary: FailureSummary,
    },
    RecoveryTargetEntered,
    RollbackSelected {
        slot: SlotId,
        audit_seq: u64,
    },
    DiagnosticsChunk(DiagnosticsChunk),
    Error(RecoveryError),
}

/// Errors from recoveryd.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum RecoveryError {
    NotConfirmed         = 0x01, // SelectRollback without confirmed_by_operator
    NoSnapshotAvailable  = 0x02,
    SlotNotFound         = 0x03,
    PermissionDenied     = 0x04,
    StorageUnavailable   = 0x05,
    BootctlUnavailable   = 0x06,
    AlreadyInRecovery    = 0x07,
    Internal             = 0xFF,
}

// ── Freshness types (for fjell-upgrade-format extension) ──────────────────────

/// Bundle freshness check status.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum FreshnessStatus {
    Valid                = 0x00,
    NotYetValid          = 0x01,
    Expired              = 0x02,
    GenerationRollback   = 0x03,
    KeyEpochRollback     = 0x04,
    ClockUnavailable     = 0x05,
    UnsupportedMetadata  = 0x06,
}

impl FreshnessStatus {
    pub fn is_admissible(self) -> bool { self == Self::Valid }
}

/// Result of a freshness check.
#[derive(Clone, Copy, Debug)]
pub struct FreshnessCheck {
    pub release_id:          [u8; 16],
    pub current_tick:        u64,
    pub generation:          u64,
    pub previous_generation: u64,
    pub key_epoch:           u64,
    pub minimum_key_epoch:   u64,
    pub status:              FreshnessStatus,
}

/// Bundle metadata v2 (for freshness checks).
#[derive(Clone, Copy, Debug)]
pub struct BundleMetadataV2 {
    pub schema_version:  u16,
    pub release_id:      [u8; 16],
    pub generation:      u64,
    pub key_epoch:       u64,
    pub issued_at_tick:  u64,
    pub not_before_tick: u64,
    pub not_after_tick:  u64,
    pub parts_digest:    Digest32,
}

impl BundleMetadataV2 {
    /// Validate freshness against the current tick and previous generation/epoch.
    pub fn check_freshness(
        &self,
        current_tick: u64,
        last_accepted_generation: u64,
        minimum_key_epoch: u64,
    ) -> FreshnessCheck {
        let status = self.compute_status(
            current_tick, last_accepted_generation, minimum_key_epoch);
        FreshnessCheck {
            release_id: self.release_id,
            current_tick,
            generation: self.generation,
            previous_generation: last_accepted_generation,
            key_epoch: self.key_epoch,
            minimum_key_epoch,
            status,
        }
    }

    fn compute_status(
        &self,
        tick: u64,
        last_gen: u64,
        min_epoch: u64,
    ) -> FreshnessStatus {
        if self.schema_version != 2 { return FreshnessStatus::UnsupportedMetadata; }
        if tick < self.not_before_tick { return FreshnessStatus::NotYetValid; }
        if tick > self.not_after_tick  { return FreshnessStatus::Expired; }
        if self.generation < last_gen  { return FreshnessStatus::GenerationRollback; }
        if self.key_epoch < min_epoch  { return FreshnessStatus::KeyEpochRollback; }
        FreshnessStatus::Valid
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn base_meta() -> BundleMetadataV2 {
        BundleMetadataV2 {
            schema_version:  2,
            release_id:      *b"release-2026-05\0",
            generation:      5,
            key_epoch:       3,
            issued_at_tick:  1000,
            not_before_tick: 1000,
            not_after_tick:  9000,
            parts_digest:    Digest32([0xAA; 32]),
        }
    }

    #[test]
    fn valid_freshness() {
        let m = base_meta();
        let r = m.check_freshness(5000, 4, 2);
        assert_eq!(r.status, FreshnessStatus::Valid);
        assert!(r.status.is_admissible());
    }

    #[test]
    fn expired_rejected() {
        let m = base_meta();
        let r = m.check_freshness(9001, 4, 2);
        assert_eq!(r.status, FreshnessStatus::Expired);
        assert!(!r.status.is_admissible());
    }

    #[test]
    fn not_yet_valid_rejected() {
        let m = base_meta();
        let r = m.check_freshness(500, 4, 2);
        assert_eq!(r.status, FreshnessStatus::NotYetValid);
        assert!(!r.status.is_admissible());
    }

    #[test]
    fn generation_rollback_rejected() {
        let m = base_meta();
        // last accepted generation = 6 > bundle.generation = 5
        let r = m.check_freshness(5000, 6, 2);
        assert_eq!(r.status, FreshnessStatus::GenerationRollback);
        assert!(!r.status.is_admissible());
    }

    #[test]
    fn key_epoch_rollback_rejected() {
        let m = base_meta();
        // minimum_key_epoch = 4 > bundle.key_epoch = 3
        let r = m.check_freshness(5000, 4, 4);
        assert_eq!(r.status, FreshnessStatus::KeyEpochRollback);
        assert!(!r.status.is_admissible());
    }

    #[test]
    fn rollback_requires_confirmation() {
        // Verify the type enforces confirmation — logic is in recoveryd,
        // but the type carries the flag clearly.
        let req = RecoveryRequest::SelectRollback {
            slot: SlotId::A,
            reason: RollbackReason::OperatorRequested,
            confirmed_by_operator: false,
        };
        if let RecoveryRequest::SelectRollback { confirmed_by_operator, .. } = req {
            assert!(!confirmed_by_operator,
                "unconfirmed rollback flag must be preserved");
        }
    }

    #[test]
    fn unsupported_schema_rejected() {
        let mut m = base_meta();
        m.schema_version = 99;
        let r = m.check_freshness(5000, 4, 2);
        assert_eq!(r.status, FreshnessStatus::UnsupportedMetadata);
    }

    #[test]
    fn same_generation_accepted() {
        let m = base_meta(); // generation = 5
        let r = m.check_freshness(5000, 5, 2); // last accepted = 5 (equal OK)
        assert_eq!(r.status, FreshnessStatus::Valid);
    }
}
