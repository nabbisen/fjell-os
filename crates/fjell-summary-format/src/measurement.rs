//! `MeasurementSummary` wire type (RFC v0.7-003 §6.1).

use fjell_measure_format::Digest32;

pub const MSUMMARY_SCHEMA_VERSION: u16 = 1;
pub const MAX_KIND_COUNTS: usize = 16;

/// Per-`MeasurementKind` event tally.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct MeasurementKindCount {
    pub kind:  u8,
    pub count: u32,
}

/// Exported snapshot of the measurement chain head and per-kind tallies.
#[derive(Clone, Copy, Debug)]
pub struct MeasurementSummary {
    pub schema_version:     u16,
    pub source_node_id:     [u8; 16],
    pub issued_tick:        u64,
    pub head_seq:           u64,
    pub head_chain_digest:  Digest32,
    pub kind_count:         u8,
    pub kind_counts:        [MeasurementKindCount; MAX_KIND_COUNTS],
    pub policy_digest:      Digest32,
    /// Canonical digest; compute with `measurement_summary_digest` before storing.
    pub summary_digest:     Digest32,
}

impl MeasurementSummary {
    pub fn new(
        source_node_id:    [u8; 16],
        issued_tick:       u64,
        head_seq:          u64,
        head_chain_digest: Digest32,
        policy_digest:     Digest32,
    ) -> Self {
        Self {
            schema_version:    MSUMMARY_SCHEMA_VERSION,
            source_node_id,
            issued_tick,
            head_seq,
            head_chain_digest,
            kind_count:        0,
            kind_counts:       [MeasurementKindCount::default(); MAX_KIND_COUNTS],
            policy_digest,
            summary_digest:    Digest32([0u8; 32]),
        }
    }

    /// Add a per-kind event count. Returns `Err` if the table is full.
    pub fn add_kind_count(&mut self, kind: u8, count: u32) -> Result<(), ()> {
        if self.kind_count as usize >= MAX_KIND_COUNTS { return Err(()); }
        self.kind_counts[self.kind_count as usize] = MeasurementKindCount { kind, count };
        self.kind_count += 1;
        Ok(())
    }
}
