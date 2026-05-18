//! System snapshot types for Fjell OS M7.
#![no_std]

/// Unique identifier for a snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotId(pub u64);

/// Reason a snapshot was created.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotReason {
    Boot,
    PreUpgrade,
    PostConfirmation,
    Rollback,
    Periodic,
}

/// Compact digest of the system state at snapshot time.
///
/// RFC 041 (v0.2.0): `audit_last_seq` and `audit_dropped_count` fields added
/// so that evidence continuity can be verified at rollback and attestation time.
#[derive(Clone, Copy)]
pub struct SnapshotDigest {
    pub slot:          u8,
    pub release_hash:  [u8; 8],
    pub rootfs_hash:   [u8; 8],
    pub policy_hash:   [u8; 8],
    pub store_seq:     u64,
    /// Sequence number of the last audit record persisted before this snapshot
    /// (RFC 041 §"Snapshot extension").  0 if no audit records have been
    /// persisted yet.
    pub audit_last_seq:       u64,
    /// Total audit records dropped (kernel ring overflows) since boot, as
    /// reported by the last `sys_audit_drain` call.
    pub audit_dropped_count:  u64,
    /// Legacy field retained for backward compatibility; audit_seq is
    /// superseded by audit_last_seq.
    pub audit_seq:     u64,
}

impl SnapshotDigest {
    /// Build a snapshot digest without audit state (backward compat).
    pub const fn current(slot: u8, store_seq: u64) -> Self {
        SnapshotDigest {
            slot,
            release_hash: *b"REL_HASH",
            rootfs_hash:  *b"RFS_HASH",
            policy_hash:  *b"POL_HASH",
            store_seq,
            audit_seq: 0,
            audit_last_seq: 0,
            audit_dropped_count: 0,
        }
    }

    /// Build a snapshot digest with full RFC 041 audit evidence state.
    pub fn with_audit(
        slot:                u8,
        store_seq:           u64,
        audit_last_seq:      u64,
        audit_dropped_count: u64,
    ) -> Self {
        SnapshotDigest {
            slot,
            release_hash: *b"REL_HASH",
            rootfs_hash:  *b"RFS_HASH",
            policy_hash:  *b"POL_HASH",
            store_seq,
            audit_seq: audit_last_seq,
            audit_last_seq,
            audit_dropped_count,
        }
    }

    /// True if there are gaps in the audit evidence (records were dropped).
    pub fn has_audit_gaps(&self) -> bool {
        self.audit_dropped_count > 0
    }
}

/// A system snapshot record.
#[derive(Clone, Copy)]
pub struct SystemSnapshot {
    pub id:      SnapshotId,
    pub reason:  SnapshotReason,
    pub digest:  SnapshotDigest,
    pub seq:     u64,
}

impl SystemSnapshot {
    pub fn new(id: u64, reason: SnapshotReason, slot: u8, store_seq: u64) -> Self {
        SystemSnapshot {
            id: SnapshotId(id), reason,
            digest: SnapshotDigest::current(slot, store_seq),
            seq: store_seq,
        }
    }

    /// Build a snapshot with full RFC 041 audit evidence state.
    pub fn new_with_audit(
        id:                  u64,
        reason:              SnapshotReason,
        slot:                u8,
        store_seq:           u64,
        audit_last_seq:      u64,
        audit_dropped_count: u64,
    ) -> Self {
        SystemSnapshot {
            id: SnapshotId(id), reason,
            digest: SnapshotDigest::with_audit(slot, store_seq, audit_last_seq, audit_dropped_count),
            seq: store_seq,
        }
    }
    pub fn reason_str(&self) -> &'static str {
        match self.reason {
            SnapshotReason::Boot             => "boot",
            SnapshotReason::PreUpgrade       => "pre-upgrade",
            SnapshotReason::PostConfirmation => "post-confirmation",
            SnapshotReason::Rollback         => "rollback",
            SnapshotReason::Periodic         => "periodic",
        }
    }
}

// ── RFC 041: evidence continuity verification ─────────────────────────────────

/// Error returned when audit evidence continuity cannot be confirmed
/// at rollback or attestation time (RFC 041 §"Continuity check").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidenceGapError {
    /// Records were dropped before this snapshot was taken.
    DroppedRecords {
        count: u64,
    },
    /// The audit sequence in this snapshot is earlier than expected,
    /// implying records were lost between two consecutive snapshots.
    SequenceRegression {
        expected_at_least: u64,
        found: u64,
    },
    /// No audit records at all — may indicate auditd never ran.
    NoAuditState,
}

impl SystemSnapshot {
    /// Verify that the snapshot's audit evidence is continuous relative to
    /// `prev`, the immediately preceding snapshot (RFC 041 §"Continuity check").
    ///
    /// Returns `Ok(())` if evidence is continuous; `Err` if there is a gap.
    pub fn verify_evidence_continuity(
        &self,
        prev: Option<&SystemSnapshot>,
    ) -> Result<(), EvidenceGapError> {
        if self.digest.audit_dropped_count > 0 {
            return Err(EvidenceGapError::DroppedRecords {
                count: self.digest.audit_dropped_count,
            });
        }
        if let Some(p) = prev {
            if self.digest.audit_last_seq < p.digest.audit_last_seq {
                return Err(EvidenceGapError::SequenceRegression {
                    expected_at_least: p.digest.audit_last_seq,
                    found: self.digest.audit_last_seq,
                });
            }
        }
        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_with_audit_roundtrip() {
        let s = SystemSnapshot::new_with_audit(1, SnapshotReason::Boot, 0, 100, 50, 0);
        assert_eq!(s.digest.audit_last_seq, 50);
        assert_eq!(s.digest.audit_dropped_count, 0);
        assert!(!s.digest.has_audit_gaps());
    }

    #[test]
    fn has_audit_gaps_detects_drops() {
        let s = SystemSnapshot::new_with_audit(1, SnapshotReason::Boot, 0, 100, 50, 3);
        assert!(s.digest.has_audit_gaps());
    }

    #[test]
    fn continuity_ok_when_no_drops_and_sequence_advances() {
        let s1 = SystemSnapshot::new_with_audit(1, SnapshotReason::Boot, 0, 100, 100, 0);
        let s2 = SystemSnapshot::new_with_audit(2, SnapshotReason::Periodic, 0, 200, 200, 0);
        assert!(s2.verify_evidence_continuity(Some(&s1)).is_ok());
    }

    #[test]
    fn continuity_err_when_drops_present() {
        let s = SystemSnapshot::new_with_audit(1, SnapshotReason::Boot, 0, 100, 50, 2);
        assert!(matches!(
            s.verify_evidence_continuity(None),
            Err(EvidenceGapError::DroppedRecords { count: 2 })
        ));
    }

    #[test]
    fn continuity_err_on_sequence_regression() {
        let s1 = SystemSnapshot::new_with_audit(1, SnapshotReason::Boot, 0, 100, 200, 0);
        let s2 = SystemSnapshot::new_with_audit(2, SnapshotReason::Periodic, 0, 200, 150, 0);
        assert!(matches!(
            s2.verify_evidence_continuity(Some(&s1)),
            Err(EvidenceGapError::SequenceRegression { expected_at_least: 200, found: 150 })
        ));
    }
}
