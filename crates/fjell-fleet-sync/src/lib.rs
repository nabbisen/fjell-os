//! # `fjell-fleet-sync`
//!
//! Fleet partition state machine and reconciliation types (RFC-v0.13).
//!
//! ## Three facilities
//!
//! 1. **`FleetState`** — the partition FSM (§13-002):
//!    `Healthy → Suspect → Partitioned → Reconciling → Healthy`
//!
//! 2. **`ReconcileManifest`** — signed artefact that the coordinator
//!    produces after merging partitioned-side state (§13-002 §4).
//!
//! 3. **`ReattestManifest`** — summary of a bulk re-attestation run
//!    (§13-004 §2).

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;
use alloc::vec::Vec;
use alloc::string::String;

// ── Fleet partition state machine ─────────────────────────────────────────────

/// Lifecycle state of a fleet node's connectivity to its coordinator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FleetState {
    /// Heartbeats arriving on schedule. Wire 0x01.
    Healthy     = 0x01,
    /// One or more heartbeats missed but partition_threshold not reached. 0x02.
    Suspect     = 0x02,
    /// partition_threshold exceeded; operating without coordinator. 0x03.
    Partitioned = 0x03,
    /// Link restored; coordinator is merging state. 0x04.
    Reconciling = 0x04,
}

impl FleetState {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::Healthy),
            0x02 => Some(Self::Suspect),
            0x03 => Some(Self::Partitioned),
            0x04 => Some(Self::Reconciling),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Healthy     => "Healthy",
            Self::Suspect     => "Suspect",
            Self::Partitioned => "Partitioned",
            Self::Reconciling => "Reconciling",
        }
    }
}

/// Guard: is this a valid state transition?
pub fn is_valid_fleet_transition(from: FleetState, to: FleetState) -> bool {
    use FleetState::*;
    matches!((from, to),
        (Healthy,     Suspect)     |
        (Suspect,     Healthy)     |   // heartbeat restored
        (Suspect,     Partitioned) |
        (Partitioned, Reconciling) |
        (Reconciling, Healthy)
    )
}

// ── Reconcile manifest ────────────────────────────────────────────────────────

/// An authority-class decision for a record produced during a partition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ReconcileDecision {
    /// Record accepted into the authoritative state.  0x01.
    Accepted  = 0x01,
    /// Record refused as authority-conflicting; preserved as evidence. 0x02.
    Rejected  = 0x02,
}

impl ReconcileDecision {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v { 0x01 => Some(Self::Accepted), 0x02 => Some(Self::Rejected), _ => None }
    }
}

/// Digest of a single record processed during reconciliation.
#[derive(Clone)]
pub struct ReconcileEntry {
    /// SHA-256 of the canonical record bytes (first 32 bytes used as ID).
    pub record_digest: [u8; 32],
    pub decision:      ReconcileDecision,
    /// Reason code for `Rejected` entries.
    pub reason_code:   u8,
}

/// The coordinator's signed reconciliation manifest (RFC-v0.13-002 §4).
///
/// Produced after merging a partitioned member's state.  Members apply
/// the manifest after verifying the coordinator's signature.
pub struct ReconcileManifest {
    pub schema:          u16,
    /// Monotonic per-coordinator reconciliation counter.
    pub seq:             u64,
    /// Node ID of the coordinator.
    pub coordinator_id:  [u8; 16],
    /// Node ID that was partitioned.
    pub member_id:       [u8; 16],
    /// Partition start tick (coordinator monotonic ns).
    pub partition_start: u64,
    /// Reconcile complete tick.
    pub reconcile_at:    u64,
    pub entries:         Vec<ReconcileEntry>,
    /// Ed25519 signature over the canonical prefix.
    pub signature:       [u8; 64],
}

impl ReconcileManifest {
    pub const MAGIC: &'static [u8; 4] = b"FREC";
    pub const SCHEMA: u16 = 1;

    pub fn new(
        seq: u64,
        coordinator_id: [u8; 16],
        member_id: [u8; 16],
        partition_start: u64,
        reconcile_at: u64,
        entries: Vec<ReconcileEntry>,
    ) -> Self {
        Self {
            schema: Self::SCHEMA,
            seq, coordinator_id, member_id,
            partition_start, reconcile_at, entries,
            signature: [0u8; 64],
        }
    }

    pub fn accepted_count(&self) -> usize {
        self.entries.iter().filter(|e| e.decision == ReconcileDecision::Accepted).count()
    }

    pub fn rejected_count(&self) -> usize {
        self.entries.iter().filter(|e| e.decision == ReconcileDecision::Rejected).count()
    }
}

// ── Coordinator promotion ─────────────────────────────────────────────────────

/// Signed record promoting a member to coordinator (RFC-v0.13-005 §4).
///
/// Requires the `TrustAnchorRoot` signature.
pub struct CoordinatorPromotion {
    pub schema:              u16,
    /// Node being promoted.
    pub new_coordinator_id:  [u8; 16],
    /// Previous coordinator (may be zeroed if it is permanently unavailable).
    pub previous_coord_id:   [u8; 16],
    pub promoted_at_ns:      u64,
    /// Fleet size at time of promotion.
    pub surviving_members:   u16,
    /// Signed by the `TrustAnchorRoot` key (RFC-v0.13-003 §5).
    pub signature:           [u8; 64],
    pub signer_key_id:       [u8; 16],
}

impl CoordinatorPromotion {
    pub const MAGIC: &'static [u8; 4] = b"FPRO";
    pub const SCHEMA: u16 = 1;
}

// ── Bulk re-attestation manifest (RFC-v0.13-004) ──────────────────────────────

/// Per-node result in a re-attestation run.
#[derive(Clone)]
pub struct NodeReattestResult {
    pub node_id:    [u8; 16],
    /// `true` if the node responded within the window.
    pub responded:  bool,
    /// `true` if the response contained a valid signed attestation.
    pub verified:   bool,
}

/// Trigger reason for a bulk re-attestation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ReattestReason {
    Scheduled  = 1,
    Rotation   = 2,
    Incident   = 3,
    Refresh    = 4,
}

impl ReattestReason {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::Scheduled),
            2 => Some(Self::Rotation),
            3 => Some(Self::Incident),
            4 => Some(Self::Refresh),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Scheduled => "scheduled",
            Self::Rotation  => "rotation",
            Self::Incident  => "incident",
            Self::Refresh   => "refresh",
        }
    }
}

/// Summary of a completed bulk re-attestation run.
pub struct ReattestManifest {
    pub schema:          u16,
    pub initiated_at_ns: u64,
    pub completed_at_ns: u64,
    pub reason:          ReattestReason,
    pub fleet_size:      u32,
    pub attempted:       u32,
    pub succeeded:       u32,
    pub timed_out:       u32,
    pub refused:         u32,
    pub per_node:        Vec<NodeReattestResult>,
    pub signature:       [u8; 64],
}

impl ReattestManifest {
    pub const SCHEMA: u16 = 1;

    pub fn pass_rate_pct(&self) -> u32 {
        if self.attempted == 0 { return 0; }
        self.succeeded * 100 / self.attempted
    }
}

// ── Semantic summary consistency checker (RFC-v0.13-005 §3) ──────────────────

/// A summary consistency check failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsistencyError {
    SyncSeqRegression { expected_min: u64, got: u64 },
    KeyEpochRegression { previous: u32, got: u32 },
    UnknownBundleDigest { digest_prefix: [u8; 8] },
    InvalidLifecycleTransition { from: u8, to: u8 },
    BootCountRegression { previous: u32, got: u32 },
}

/// Run static + temporal consistency checks on a new summary.
///
/// `prev_seq` is the `sync_seq` from the last accepted summary for the node
/// (0 if first summary). Returns all detected violations.
pub fn check_summary_consistency(
    new_seq:          u64,
    new_key_epoch:    u32,
    new_boot_count:   u32,
    bundle_lifecycle: u8,
    prev_seq:         u64,
    prev_key_epoch:   u32,
    prev_boot_count:  u32,
    prev_lifecycle:   u8,
    bundle_digest:    [u8; 32],
    known_bundles:    &[[u8; 32]],
) -> Vec<ConsistencyError> {
    let mut errors = Vec::new();

    // Temporal: seq must advance
    if prev_seq > 0 && new_seq <= prev_seq {
        errors.push(ConsistencyError::SyncSeqRegression {
            expected_min: prev_seq + 1, got: new_seq,
        });
    }

    // Temporal: key epoch must not decrease
    if new_key_epoch < prev_key_epoch {
        errors.push(ConsistencyError::KeyEpochRegression {
            previous: prev_key_epoch, got: new_key_epoch,
        });
    }

    // Temporal: boot count must not decrease (unless it IS a boot record)
    if new_boot_count < prev_boot_count {
        errors.push(ConsistencyError::BootCountRegression {
            previous: prev_boot_count, got: new_boot_count,
        });
    }

    // Static: bundle digest must be known
    let bundle_known = known_bundles.iter().any(|k| k == &bundle_digest);
    if !bundle_known {
        let mut prefix = [0u8; 8];
        prefix.copy_from_slice(&bundle_digest[..8]);
        errors.push(ConsistencyError::UnknownBundleDigest { digest_prefix: prefix });
    }

    // Static: lifecycle transition must be valid per RFC-v0.9-004 FSM
    // Valid transitions: 1→2, 2→3, 3→4, 4→5, 4→6; same state also allowed.
    let valid = prev_lifecycle == 0  // first summary
        || bundle_lifecycle == prev_lifecycle
        || matches!((prev_lifecycle, bundle_lifecycle),
            (1,2) | (2,3) | (3,4) | (4,5) | (4,6));
    if !valid {
        errors.push(ConsistencyError::InvalidLifecycleTransition {
            from: prev_lifecycle, to: bundle_lifecycle,
        });
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── FleetState FSM ─────────────────────────────────────────────────────────

    #[test]
    fn valid_transitions() {
        assert!(is_valid_fleet_transition(FleetState::Healthy,     FleetState::Suspect));
        assert!(is_valid_fleet_transition(FleetState::Suspect,     FleetState::Healthy));
        assert!(is_valid_fleet_transition(FleetState::Suspect,     FleetState::Partitioned));
        assert!(is_valid_fleet_transition(FleetState::Partitioned, FleetState::Reconciling));
        assert!(is_valid_fleet_transition(FleetState::Reconciling, FleetState::Healthy));
    }

    #[test]
    fn invalid_transitions() {
        assert!(!is_valid_fleet_transition(FleetState::Healthy,     FleetState::Reconciling));
        assert!(!is_valid_fleet_transition(FleetState::Partitioned, FleetState::Healthy));
        assert!(!is_valid_fleet_transition(FleetState::Healthy,     FleetState::Partitioned));
    }

    #[test]
    fn state_round_trip() {
        for s in [FleetState::Healthy, FleetState::Suspect,
                  FleetState::Partitioned, FleetState::Reconciling] {
            assert_eq!(FleetState::from_u8(s as u8), Some(s));
        }
        assert!(FleetState::from_u8(0x00).is_none());
    }

    // ── ReconcileManifest ──────────────────────────────────────────────────────

    #[test]
    fn reconcile_counts() {
        let entries = vec![
            ReconcileEntry { record_digest: [0u8; 32], decision: ReconcileDecision::Accepted, reason_code: 0 },
            ReconcileEntry { record_digest: [1u8; 32], decision: ReconcileDecision::Rejected, reason_code: 1 },
            ReconcileEntry { record_digest: [2u8; 32], decision: ReconcileDecision::Accepted, reason_code: 0 },
        ];
        let m = ReconcileManifest::new(1, [0u8;16], [1u8;16], 0, 100, entries);
        assert_eq!(m.accepted_count(), 2);
        assert_eq!(m.rejected_count(), 1);
    }

    // ── ReattestManifest ───────────────────────────────────────────────────────

    #[test]
    fn pass_rate() {
        let m = ReattestManifest {
            schema: ReattestManifest::SCHEMA,
            initiated_at_ns: 0, completed_at_ns: 100,
            reason: ReattestReason::Scheduled,
            fleet_size: 10, attempted: 10, succeeded: 8,
            timed_out: 1, refused: 1,
            per_node: Vec::new(), signature: [0u8; 64],
        };
        assert_eq!(m.pass_rate_pct(), 80);
    }

    // ── Consistency checker ────────────────────────────────────────────────────

    const KNOWN: [[u8; 32]; 1] = [[0xABu8; 32]];

    fn good_check(seq: u64) -> Vec<ConsistencyError> {
        check_summary_consistency(seq, 1, 1, 4, seq-1, 1, 1, 3, [0xABu8; 32], &KNOWN)
    }

    #[test]
    fn consistent_summary_passes() {
        assert!(good_check(2).is_empty());
    }

    #[test]
    fn seq_regression_detected() {
        let errs = check_summary_consistency(1, 1, 1, 4, 5, 1, 1, 3, [0xABu8; 32], &KNOWN);
        assert!(errs.iter().any(|e| matches!(e, ConsistencyError::SyncSeqRegression { .. })));
    }

    #[test]
    fn unknown_bundle_detected() {
        let errs = check_summary_consistency(2, 1, 1, 4, 1, 1, 1, 3, [0xCCu8; 32], &KNOWN);
        assert!(errs.iter().any(|e| matches!(e, ConsistencyError::UnknownBundleDigest { .. })));
    }

    #[test]
    fn invalid_lifecycle_transition_detected() {
        // 5 (Confirmed) → 3 (Committed) is invalid
        let errs = check_summary_consistency(2, 1, 1, 3, 1, 1, 1, 5, [0xABu8; 32], &KNOWN);
        assert!(errs.iter().any(|e| matches!(e, ConsistencyError::InvalidLifecycleTransition { .. })));
    }

    #[test]
    fn key_epoch_regression_detected() {
        let errs = check_summary_consistency(2, 1, 1, 4, 1, 3, 1, 3, [0xABu8; 32], &KNOWN);
        assert!(errs.iter().any(|e| matches!(e, ConsistencyError::KeyEpochRegression { .. })));
    }

    #[test]
    fn reattest_reason_labels() {
        assert_eq!(ReattestReason::Scheduled.label(), "scheduled");
        assert_eq!(ReattestReason::Incident.label(),  "incident");
    }
}
