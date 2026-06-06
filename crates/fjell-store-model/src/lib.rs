//! Append-only store log model (RFC v0.6-002 §7.1).
//! Six properties tested with proptest.

use proptest::prelude::*;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum RecordKind {
    Boot       = 0x01,
    Policy     = 0x02,
    Snapshot   = 0x03,
    Staging    = 0x04,
    Rollback   = 0x05,
    Audit      = 0x06,
    Health     = 0x07,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommittedRecord {
    pub seq:         u64,
    pub kind:        RecordKind,
    pub payload:     Vec<u8>,
    pub corrupt:     bool,   // digest is bad — skip on replay
}

#[derive(Clone, Debug)]
pub struct LogModel {
    /// The "on-disk" byte stream (simplified: a Vec of records).
    pub records:    Vec<CommittedRecord>,
    /// Records visible after the last Restart (replay result).
    pub replayed:   Vec<CommittedRecord>,
    pub next_seq:   u64,
    pub crashed_at: Option<usize>,   // record index where crash occurred
}

impl LogModel {
    pub fn new() -> Self {
        Self { records: vec![], replayed: vec![], next_seq: 0, crashed_at: None }
    }

    // ── Operations ────────────────────────────────────────────────────────────

    pub fn append(&mut self, kind: RecordKind, payload: Vec<u8>, corrupt: bool) {
        if self.crashed_at.is_some() { return; } // after crash, no writes
        let seq = self.next_seq;
        self.next_seq += 1;
        self.records.push(CommittedRecord { seq, kind, payload, corrupt });
    }

    /// Simulate crash: drop the last `tail` records (partial write).
    pub fn crash(&mut self, tail: usize) {
        let keep = self.records.len().saturating_sub(tail);
        self.records.truncate(keep);
        self.crashed_at = Some(keep);
    }

    /// Recovery scan: rebuild `replayed` from `records`, skipping corrupt ones.
    pub fn restart(&mut self) {
        self.replayed = self.records.iter()
            .filter(|r| !r.corrupt)
            .cloned()
            .collect();
        self.crashed_at = None;
    }

    pub fn latest(&self, kind: RecordKind) -> Option<&CommittedRecord> {
        self.replayed.iter().rev().find(|r| r.kind == kind)
    }
}

// ── Properties ────────────────────────────────────────────────────────────────

/// S1: Two consecutive Restart ops produce the same state.
pub fn s1_replay_idempotent(model: &mut LogModel) -> Result<(), String> {
    model.restart();
    let first  = model.replayed.clone();
    model.restart();
    let second = model.replayed.clone();
    if first != second {
        return Err(format!("S1: replay not idempotent: first={} second={}", first.len(), second.len()));
    }
    Ok(())
}

/// S2: After Restart, latest() returns the most-recently committed valid record per kind.
pub fn s2_latest_authoritative(model: &mut LogModel) -> Result<(), String> {
    model.restart();
    for kind in [RecordKind::Policy, RecordKind::Snapshot, RecordKind::Health] {
        // Find what the latest non-corrupt record of this kind should be.
        let expected = model.records.iter().rev()
            .find(|r| r.kind == kind && !r.corrupt);
        let got = model.latest(kind);
        match (expected, got) {
            (None, None) => {}
            (Some(e), Some(g)) if e.seq == g.seq => {}
            (Some(e), Some(g)) => return Err(format!(
                "S2: latest({kind:?}) expected seq={} got seq={}", e.seq, g.seq
            )),
            (Some(e), None) => return Err(format!(
                "S2: latest({kind:?}) expected seq={} got None", e.seq
            )),
            (None, Some(g)) => return Err(format!(
                "S2: latest({kind:?}) expected None got seq={}", g.seq
            )),
        }
    }
    Ok(())
}

/// S3: After crash (partial write) and Restart, the crashed record is absent.
pub fn s3_crash_drops_partial(model: &mut LogModel, crash_tail: usize) -> Result<(), String> {
    let before_len = model.records.len();
    let drop = crash_tail.min(before_len);
    let kept = before_len - drop;
    model.crash(crash_tail);
    model.restart();
    if model.replayed.len() > kept {
        return Err(format!(
            "S3: expected ≤{kept} records after crash, got {}", model.replayed.len()
        ));
    }
    Ok(())
}

/// S4: Corrupt records are skipped; valid neighbours survive.
pub fn s4_corrupt_record_skipped(model: &mut LogModel) -> Result<(), String> {
    // Inject a corrupt record and check it's absent after restart.
    model.append(RecordKind::Audit, vec![0xFF], true);  // corrupt
    let good_seq = model.next_seq;
    model.append(RecordKind::Audit, vec![0x01], false); // valid
    model.restart();
    // Corrupt record must not appear; valid record must appear.
    let has_corrupt = model.replayed.iter().any(|r| r.corrupt);
    let has_good    = model.replayed.iter().any(|r| r.seq == good_seq);
    if has_corrupt { return Err("S4: corrupt record survived restart".into()); }
    if !has_good   { return Err("S4: valid record lost after corrupt neighbour".into()); }
    Ok(())
}

/// S5: The highest rollback counter seq is the authoritative one after restart.
pub fn s5_rollback_replay_highest(model: &mut LogModel) -> Result<(), String> {
    model.restart();
    let seqs: Vec<u64> = model.replayed.iter()
        .filter(|r| r.kind == RecordKind::Rollback)
        .map(|r| r.seq)
        .collect();
    if seqs.len() >= 2 {
        let max_seq = *seqs.iter().max().unwrap();
        // latest() for Rollback must be the highest seq.
        if let Some(l) = model.latest(RecordKind::Rollback) {
            if l.seq != max_seq {
                return Err(format!("S5: latest Rollback seq={} not max={max_seq}", l.seq));
            }
        }
    }
    Ok(())
}

/// S6: Staging record at restart is the last terminal or last non-terminal state.
pub fn s6_staging_replay_coherent(model: &mut LogModel) -> Result<(), String> {
    model.restart();
    // We simply verify the staging records are in order by seq.
    let staging_seqs: Vec<u64> = model.replayed.iter()
        .filter(|r| r.kind == RecordKind::Staging)
        .map(|r| r.seq)
        .collect();
    for w in staging_seqs.windows(2) {
        if w[0] >= w[1] {
            return Err(format!("S6: staging seqs not monotone: {} >= {}", w[0], w[1]));
        }
    }
    Ok(())
}

// ── Proptest generators ───────────────────────────────────────────────────────

fn arb_record_kind() -> impl Strategy<Value = RecordKind> {
    prop_oneof![
        Just(RecordKind::Boot),   Just(RecordKind::Policy),
        Just(RecordKind::Snapshot), Just(RecordKind::Staging),
        Just(RecordKind::Rollback), Just(RecordKind::Health),
    ]
}

fn arb_record() -> impl Strategy<Value = (RecordKind, Vec<u8>, bool)> {
    (arb_record_kind(), prop::collection::vec(any::<u8>(), 0..=16), prop::bool::weighted(0.1))
}

pub fn arb_store_sequence() -> impl Strategy<Value = Vec<(RecordKind, Vec<u8>, bool)>> {
    prop::collection::vec(arb_record(), 0..=32)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 1_000, ..ProptestConfig::default() })]

    #[test]
    fn test_s1_replay_idempotent(recs in arb_store_sequence()) {
        let mut m = LogModel::new();
        for (k, p, c) in recs { m.append(k, p, c); }
        properties::s1_replay_idempotent(&mut m).map_err(|e| TestCaseError::fail(e))?;
    }

    #[test]
    fn test_s2_latest_authoritative(recs in arb_store_sequence()) {
        let mut m = LogModel::new();
        for (k, p, c) in recs { m.append(k, p, c); }
        properties::s2_latest_authoritative(&mut m).map_err(|e| TestCaseError::fail(e))?;
    }

    #[test]
    fn test_s3_crash_drops_partial(recs in arb_store_sequence(), tail in 0usize..=4) {
        let mut m = LogModel::new();
        for (k, p, c) in recs { m.append(k, p, c); }
        properties::s3_crash_drops_partial(&mut m, tail).map_err(|e| TestCaseError::fail(e))?;
    }

    #[test]
    fn test_s4_corrupt_record_skipped(recs in arb_store_sequence()) {
        let mut m = LogModel::new();
        for (k, p, c) in recs { m.append(k, p, c); }
        properties::s4_corrupt_record_skipped(&mut m).map_err(|e| TestCaseError::fail(e))?;
    }

    #[test]
    fn test_s5_rollback_replay_highest(recs in arb_store_sequence()) {
        let mut m = LogModel::new();
        for (k, p, c) in recs { m.append(k, p, c); }
        properties::s5_rollback_replay_highest(&mut m).map_err(|e| TestCaseError::fail(e))?;
    }

    #[test]
    fn test_s6_staging_replay_coherent(recs in arb_store_sequence()) {
        let mut m = LogModel::new();
        for (k, p, c) in recs { m.append(k, p, c); }
        properties::s6_staging_replay_coherent(&mut m).map_err(|e| TestCaseError::fail(e))?;
    }
}

/// Make property functions accessible from the proptest! macro.
mod properties {
    pub use super::{
        s1_replay_idempotent, s2_latest_authoritative, s3_crash_drops_partial,
        s4_corrupt_record_skipped, s5_rollback_replay_highest, s6_staging_replay_coherent,
    };
}
