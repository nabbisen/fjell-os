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
/// Conflict-domain tag on each snapshot record (RFC v0.7-004 §6.1).
///
/// IMPORTANT: The `derive(Default)` is deliberately removed.
/// v1 absent-domain decodes to `V1_DEFAULT = ForeignAuthoritative`, not
/// `LocallyConfirmed`. Use `ConflictDomain::V1_DEFAULT` explicitly in
/// decoders. See RFC-v0.7.2-002.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum ConflictDomain {
    LocallyConfirmed     = 0x01,
    ForeignAuthoritative = 0x02,
    Pending              = 0x03,
    Contested            = 0x04,
}

impl ConflictDomain {
    /// Default for v1 envelopes that omit the domain byte (RFC v0.7-004 §5.2,
    /// RFC-v0.7.2-002).
    pub const V1_DEFAULT: Self = Self::ForeignAuthoritative;

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

    pub fn push_record(&mut self, r: SnapshotRecord) -> Result<(), SnapshotError> {
        if r.body_len as usize > SNAPSHOT_RECORD_BODY_MAX {
            return Err(SnapshotError::BodyTooLarge);
        }
        if self.record_count as usize >= MAX_SNAPSHOT_RECORDS {
            return Err(SnapshotError::CapacityExhausted);
        }
        self.records[self.record_count as usize] = Some(r);
        self.record_count += 1;
        Ok(())
    }
}

/// Incremental digest writer — streams bytes into a SHA-256 hasher without
/// a fixed-size intermediate buffer (RFC-v0.7.2-002, closes C-RB-02).
pub struct DigestWriter {
    state: [u32; 8],
    buf:   [u8; 64],
    buf_len: usize,
    total:   u64,
}

impl DigestWriter {
    pub fn new() -> Self {
        // SHA-256 initial hash values (FIPS 180-4 §5.3.3)
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
            ],
            buf: [0u8; 64],
            buf_len: 0,
            total: 0,
        }
    }

    fn compress(&mut self) {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
            0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
            0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
            0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
            0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
            0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
            0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
            0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
            0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
            0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
            0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
            0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
            0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
            0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
        ];
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                self.buf[i*4], self.buf[i*4+1], self.buf[i*4+2], self.buf[i*4+3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i-15].rotate_right(7) ^ w[i-15].rotate_right(18) ^ (w[i-15] >> 3);
            let s1 = w[i-2].rotate_right(17) ^ w[i-2].rotate_right(19) ^ (w[i-2] >> 10);
            w[i] = w[i-16].wrapping_add(s0).wrapping_add(w[i-7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = h.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            h = g; g = f; f = e; e = d.wrapping_add(t1);
            d = c; c = b; b = a; a = t1.wrapping_add(t2);
        }
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }

    pub fn update(&mut self, data: &[u8]) {
        for &byte in data {
            self.buf[self.buf_len] = byte;
            self.buf_len += 1;
            self.total += 1;
            if self.buf_len == 64 {
                self.compress();
                self.buf_len = 0;
            }
        }
    }

    pub fn write_u8 (&mut self, v: u8)  { self.update(&[v]); }
    pub fn write_u16(&mut self, v: u16) { self.update(&v.to_le_bytes()); }
    pub fn write_u32(&mut self, v: u32) { self.update(&v.to_le_bytes()); }
    pub fn write_u64(&mut self, v: u64) { self.update(&v.to_le_bytes()); }

    pub fn finalize(mut self) -> Digest32 {
        // SHA-256 padding
        let bit_len = self.total * 8;
        self.update(&[0x80]);
        while self.buf_len != 56 {
            self.update(&[0x00]);
        }
        self.update(&bit_len.to_be_bytes());
        let mut out = [0u8; 32];
        for (i, &word) in self.state.iter().enumerate() {
            out[i*4..i*4+4].copy_from_slice(&word.to_be_bytes());
        }
        Digest32(out)
    }
}

/// Maximum body bytes per record. Records with larger body_len are rejected
/// on push (RFC-v0.7.2-002, closes C-M-02).
pub const SNAPSHOT_RECORD_BODY_MAX: usize = 64;

/// Typed error for snapshot push/validation (RFC-v0.7.2-002).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum SnapshotError {
    BodyTooLarge      = 0x01,
    CapacityExhausted = 0x02,
    UnknownSchema     = 0x03,
    UnknownDomain     = 0x04,
}

/// Compute the canonical `snapshot_digest` using a streaming writer —
/// no fixed-size stack buffer; cannot panic at declared capacity
/// (RFC-v0.7.2-002, closes C-RB-02).
pub fn snapshot_digest(env: &SnapshotEnvelope) -> Digest32 {
    let mut w = DigestWriter::new();
    w.update(b"FJELL-SNAPSHOT-V1");
    w.write_u16(env.schema_version);
    w.update(&env.source_identity_digest.0);
    w.write_u64(env.issued_tick);
    w.update(&env.nonce);
    w.write_u16(env.record_count);
    for i in 0..env.record_count as usize {
        if let Some(r) = &env.records[i] {
            if env.schema_version >= SNAPSHOT_ENVELOPE_V2 {
                w.write_u8(r.domain as u8);
            }
            w.write_u16(r.kind);
            w.write_u64(r.seq);
            let len = (r.body_len as usize).min(SNAPSHOT_RECORD_BODY_MAX);
            w.write_u32(len as u32);
            w.update(&r.body[..len]);
        }
    }
    w.finalize()
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
        }), Err(SnapshotError::CapacityExhausted));
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

// ── Legacy SnapshotDigest deprecation (RFC-v0.7.5-001) ───────────────────────

/// Placeholder hash constants — kept for backward compat; do NOT trust.
#[deprecated(since = "0.7.5", note = "placeholder; use Digest32 + SnapshotEnvelope")]
pub const REL_HASH: [u8; 8] = *b"REL_HASH";
#[deprecated(since = "0.7.5", note = "placeholder; use Digest32 + SnapshotEnvelope")]
pub const RFS_HASH: [u8; 8] = *b"RFS_HASH";
#[deprecated(since = "0.7.5", note = "placeholder; use Digest32 + SnapshotEnvelope")]
pub const POL_HASH: [u8; 8] = *b"POL_HASH";

// ── RFC-v0.7.2-002 acceptance tests ──────────────────────────────────────────

#[cfg(test)]
mod rfc_v072_002_tests {
    use super::*;
    use fjell_measure_format::Digest32;

    // SNAPSHOT:DIGEST_FULL_CAPACITY_NO_PANIC
    #[test]
    fn digest_full_capacity_no_panic() {
        let mut env = SnapshotEnvelope::new_v2(Digest32([0xAAu8; 32]), 1, [0x01u8; 16]);
        for i in 0..MAX_SNAPSHOT_RECORDS {
            env.push_record(SnapshotRecord {
                domain:   ConflictDomain::LocallyConfirmed,
                kind:     i as u16,
                seq:      i as u64,
                body:     [0xFFu8; 64],
                body_len: 64,
            }).unwrap();
        }
        // This must never panic, regardless of envelope size.
        let d = snapshot_digest(&env);
        assert_ne!(d.0, [0u8; 32]);
    }

    // SNAPSHOT:BODY_LEN_OVER_64_REJECTED
    #[test]
    fn body_len_over_max_rejected() {
        let mut env = SnapshotEnvelope::new_v2(Digest32([0u8; 32]), 0, [0u8; 16]);
        let r = SnapshotRecord {
            domain: ConflictDomain::Pending,
            kind: 1, seq: 1,
            body: [0u8; 64],
            body_len: (SNAPSHOT_RECORD_BODY_MAX + 1) as u32,
        };
        assert_eq!(env.push_record(r), Err(SnapshotError::BodyTooLarge));
    }

    // SNAPSHOT:V1_MISSING_DOMAIN_FOREIGN_AUTHORITATIVE
    #[test]
    fn v1_default_is_foreign_authoritative() {
        assert_eq!(ConflictDomain::V1_DEFAULT, ConflictDomain::ForeignAuthoritative);
        // V1_DEFAULT != LocallyConfirmed (the old Default derive value)
        assert_ne!(ConflictDomain::V1_DEFAULT, ConflictDomain::LocallyConfirmed);
    }

    #[test]
    fn snapshot_error_variants_accessible() {
        let e = SnapshotError::BodyTooLarge;
        assert_eq!(e as u8, 0x01);
        let e2 = SnapshotError::CapacityExhausted;
        assert_eq!(e2 as u8, 0x02);
    }

    #[test]
    fn digest_writer_deterministic() {
        let env = SnapshotEnvelope::new_v2(Digest32([0x42u8; 32]), 999, [0x0Fu8; 16]);
        let d1 = snapshot_digest(&env);
        let d2 = snapshot_digest(&env);
        assert_eq!(d1.0, d2.0);
    }

    #[test]
    fn digest_writer_nonzero_on_nonempty_envelope() {
        let env = SnapshotEnvelope::new_v2(Digest32([0x01u8; 32]), 1, [0x01u8; 16]);
        let d = snapshot_digest(&env);
        assert_ne!(d.0, [0u8; 32]);
    }

    #[test]
    fn capacity_exhausted_after_max_records() {
        let mut env = SnapshotEnvelope::new_v2(Digest32([0u8; 32]), 0, [0u8; 16]);
        for _ in 0..MAX_SNAPSHOT_RECORDS {
            env.push_record(SnapshotRecord {
                domain: ConflictDomain::Pending, kind: 0, seq: 0,
                body: [0u8; 64], body_len: 0,
            }).unwrap();
        }
        assert_eq!(
            env.push_record(SnapshotRecord {
                domain: ConflictDomain::Pending, kind: 0, seq: 0,
                body: [0u8; 64], body_len: 0,
            }),
            Err(SnapshotError::CapacityExhausted)
        );
    }
}
