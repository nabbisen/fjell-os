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

// ── v0.7 snapshot envelope types (RFC v0.7-002 + RFC v0.7-004) ───────────────

use fjell_measure_format::Digest32;

pub const SNAPSHOT_ENVELOPE_V1: u16 = 1;
pub const SNAPSHOT_ENVELOPE_V2: u16 = 2;  // adds domain field per RFC v0.7-004
pub const MAX_SNAPSHOT_RECORDS: usize = 64;

/// Conflict-domain tag on each record (RFC v0.7-004 §6.1).
///
/// V2 envelopes prepend a `domain u8` to every record.
/// V1 readers default missing domain to `ForeignAuthoritative`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum ConflictDomain {
    #[default]
    LocallyConfirmed     = 0x01,
    ForeignAuthoritative = 0x02,
    Pending              = 0x03,
    Contested            = 0x04,
}

impl ConflictDomain {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::LocallyConfirmed),
            0x02 => Some(Self::ForeignAuthoritative),
            0x03 => Some(Self::Pending),
            0x04 => Some(Self::Contested),
            _    => None,
        }
    }
}

/// A single record within a signed snapshot envelope.
#[derive(Clone, Debug)]
pub struct SnapshotRecord {
    /// Conflict domain (v2 only; defaults to `ForeignAuthoritative` when
    /// decoded from a v1 envelope).
    pub domain:   ConflictDomain,
    pub kind:     u16,
    pub seq:      u64,
    pub body:     [u8; 64],   // fixed-size body slot (real body truncated to 64 B)
    pub body_len: u32,
}

/// Outcome of a snapshot import attempt.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SnapshotImportOutcome {
    Accepted { records_applied: u16, records_skipped: u16 },
    Refused  { reason: SnapshotImportError },
}

/// Reason a snapshot import was refused.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum SnapshotImportError {
    SignatureFailed      = 0x01,
    IdentityNotPermitted = 0x02,
    DigestMismatch       = 0x03,
    SchemaTooNew         = 0x04,
    Expired              = 0x05,
    ReplayDetected       = 0x06,
}

/// Signed snapshot envelope (v1 or v2).
pub struct SnapshotEnvelope {
    pub schema_version:         u16,
    pub source_identity_digest: Digest32,
    pub issued_tick:            u64,
    pub nonce:                  [u8; 16],
    pub record_count:           u16,
    pub records:                [Option<SnapshotRecord>; MAX_SNAPSHOT_RECORDS],
    /// Canonical digest over all the above fields.
    pub snapshot_digest:        Digest32,
}

impl SnapshotEnvelope {
    pub fn new_v2(
        source_identity_digest: Digest32,
        issued_tick:            u64,
        nonce:                  [u8; 16],
    ) -> Self {
        Self {
            schema_version:         SNAPSHOT_ENVELOPE_V2,
            source_identity_digest,
            issued_tick,
            nonce,
            record_count:           0,
            records:                core::array::from_fn(|_| None),
            snapshot_digest:        Digest32([0u8; 32]),
        }
    }

    pub fn push_record(&mut self, r: SnapshotRecord) -> Result<(), ()> {
        if self.record_count as usize >= MAX_SNAPSHOT_RECORDS { return Err(()); }
        self.records[self.record_count as usize] = Some(r);
        self.record_count += 1;
        Ok(())
    }
}

/// Compute the canonical `snapshot_digest` (RFC v0.7-002 §6.1, amended by
/// RFC v0.7-004 §6.1 for v2 `domain` field).
pub fn snapshot_digest(env: &SnapshotEnvelope) -> Digest32 {
    let mut buf = [0u8; 4096];
    let mut pos = 0usize;

    macro_rules! w_u8  { ($v:expr) => { buf[pos] = $v; pos += 1; }; }
    macro_rules! w_u16 { ($v:expr) => { buf[pos..pos+2].copy_from_slice(&($v as u16).to_le_bytes()); pos += 2; }; }
    macro_rules! w_u32 { ($v:expr) => { buf[pos..pos+4].copy_from_slice(&($v as u32).to_le_bytes()); pos += 4; }; }
    macro_rules! w_u64 { ($v:expr) => { buf[pos..pos+8].copy_from_slice(&($v as u64).to_le_bytes()); pos += 8; }; }
    macro_rules! w_b   { ($b:expr) => { let bb: &[u8] = $b; buf[pos..pos+bb.len()].copy_from_slice(bb); pos += bb.len(); }; }

    w_b!(b"FJELL-SNAPSHOT-V1");
    w_u16!(env.schema_version);
    w_b!(&env.source_identity_digest.0);
    w_u64!(env.issued_tick);
    w_b!(&env.nonce);
    w_u16!(env.record_count);
    for i in 0..env.record_count as usize {
        if let Some(r) = &env.records[i] {
            if env.schema_version >= SNAPSHOT_ENVELOPE_V2 {
                w_u8!(r.domain as u8);
            }
            w_u16!(r.kind);
            w_u64!(r.seq);
            let len = r.body_len.min(64) as usize;
            w_u32!(len as u32);
            w_b!(&r.body[..len]);
        }
    }

    Digest32::of(&buf[..pos])
}

// ── v0.7 envelope tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod v07_tests {
    use super::*;
    use fjell_measure_format::Digest32;

    #[test]
    fn snapshot_envelope_v2_version_constant() {
        assert_eq!(SNAPSHOT_ENVELOPE_V2, 2);
    }

    #[test]
    fn conflict_domain_roundtrip() {
        for (byte, expected) in [
            (0x01u8, ConflictDomain::LocallyConfirmed),
            (0x02,   ConflictDomain::ForeignAuthoritative),
            (0x03,   ConflictDomain::Pending),
            (0x04,   ConflictDomain::Contested),
        ] {
            assert_eq!(ConflictDomain::from_u8(byte).unwrap() as u8, expected as u8);
        }
        assert!(ConflictDomain::from_u8(0xFF).is_none());
    }

    #[test]
    fn envelope_push_record_increments_count() {
        let mut env = SnapshotEnvelope::new_v2(
            Digest32([0u8; 32]), 1000, [0u8; 16],
        );
        env.push_record(SnapshotRecord {
            domain: ConflictDomain::LocallyConfirmed,
            kind: 0x0001, seq: 1,
            body: [0u8; 64], body_len: 0,
        }).unwrap();
        assert_eq!(env.record_count, 1);
    }

    #[test]
    fn envelope_push_record_rejects_at_capacity() {
        let mut env = SnapshotEnvelope::new_v2(
            Digest32([0u8; 32]), 0, [0u8; 16],
        );
        for _ in 0..MAX_SNAPSHOT_RECORDS {
            env.push_record(SnapshotRecord {
                domain: ConflictDomain::Pending,
                kind: 0, seq: 0,
                body: [0u8; 64], body_len: 0,
            }).unwrap();
        }
        assert_eq!(env.push_record(SnapshotRecord {
            domain: ConflictDomain::Pending, kind: 0, seq: 0,
            body: [0u8; 64], body_len: 0,
        }), Err(()));
    }

    #[test]
    fn snapshot_digest_nonzero() {
        let mut env = SnapshotEnvelope::new_v2(
            Digest32([0xAAu8; 32]), 42_000, [0x01u8; 16],
        );
        env.push_record(SnapshotRecord {
            domain: ConflictDomain::LocallyConfirmed,
            kind: 0x0010, seq: 7,
            body: [0u8; 64], body_len: 4,
        }).unwrap();
        let d = snapshot_digest(&env);
        assert_ne!(d.0, [0u8; 32]);
    }

    #[test]
    fn snapshot_digest_deterministic() {
        let env = SnapshotEnvelope::new_v2(Digest32([0u8; 32]), 0, [0u8; 16]);
        assert_eq!(snapshot_digest(&env).0, snapshot_digest(&env).0);
    }

    #[test]
    fn snapshot_digest_sensitive_to_record_domain() {
        let mut env1 = SnapshotEnvelope::new_v2(Digest32([0u8; 32]), 0, [0u8; 16]);
        let mut env2 = SnapshotEnvelope::new_v2(Digest32([0u8; 32]), 0, [0u8; 16]);
        env1.push_record(SnapshotRecord {
            domain: ConflictDomain::LocallyConfirmed,
            kind: 1, seq: 1, body: [0u8; 64], body_len: 0,
        }).unwrap();
        env2.push_record(SnapshotRecord {
            domain: ConflictDomain::Contested,       // different domain
            kind: 1, seq: 1, body: [0u8; 64], body_len: 0,
        }).unwrap();
        assert_ne!(snapshot_digest(&env1).0, snapshot_digest(&env2).0);
    }

    #[test]
    fn snapshot_import_error_variants() {
        let outcome = SnapshotImportOutcome::Refused {
            reason: SnapshotImportError::SignatureFailed,
        };
        assert!(matches!(
            outcome,
            SnapshotImportOutcome::Refused { reason: SnapshotImportError::SignatureFailed }
        ));
    }
}
