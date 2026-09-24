//! Canonical summary digest computations (RFC v0.7-003 §6.1).

use crate::measurement::{MSUMMARY_SCHEMA_VERSION, MeasurementSummary};
use crate::release::{RSUMMARY_SCHEMA_VERSION, ReleaseSummary};
use fjell_canon::{BufSink, Canon};
use fjell_measure_format::Digest32;

/// `SHA256("FJELL-MSUMMARY-V1" || schema || source_node_id || ...)`.
pub fn measurement_summary_digest(s: &MeasurementSummary) -> Digest32 {
    let mut sink = BufSink::<512>::new();
    write_measurement_canonical(s, &mut sink);
    Digest32::of(sink.bytes())
}

/// The canonical byte stream `measurement_summary_digest` is taken over —
/// **the function the digest is computed from and the frozen schema is
/// generated from**.
pub fn write_measurement_canonical(s: &MeasurementSummary, c: &mut dyn Canon) {
    c.domain(b"FJELL-MSUMMARY-V1");
    c.u16("schema_version", MSUMMARY_SCHEMA_VERSION);
    c.bytes("source_node_id", &s.source_node_id);
    c.u64("issued_tick", s.issued_tick);
    c.u64("head_seq", s.head_seq);
    c.bytes("head_chain_digest", &s.head_chain_digest.0);
    c.u8("kind_count", s.kind_count);
    c.each("kind_counts", s.kind_count as usize, None, &mut |c, i| {
        c.u8("kind", s.kind_counts[i].kind);
        c.u32("count", s.kind_counts[i].count);
    });
    c.bytes("policy_digest", &s.policy_digest.0);
}

/// `SHA256("FJELL-RSUMMARY-V1" || schema || source_node_id || ...)`.
pub fn release_summary_digest(s: &ReleaseSummary) -> Digest32 {
    let mut sink = BufSink::<512>::new();
    write_release_canonical(s, &mut sink);
    Digest32::of(sink.bytes())
}

/// The canonical byte stream `release_summary_digest` is taken over — **the
/// function the digest is computed from and the frozen schema is generated from**.
pub fn write_release_canonical(s: &ReleaseSummary, c: &mut dyn Canon) {
    c.domain(b"FJELL-RSUMMARY-V1");
    c.u16("schema_version", RSUMMARY_SCHEMA_VERSION);
    c.bytes("source_node_id", &s.source_node_id);
    c.u64("issued_tick", s.issued_tick);
    c.u8("channel_count", s.channel_count);
    c.each("channels", s.channel_count as usize, None, &mut |c, i| {
        let ch = &s.channels[i];
        c.bytes("channel_id", &ch.channel_id);
        c.u64("current_counter", ch.current_counter);
        c.u64("min_counter", ch.min_counter);
        c.u32("active_anchor_epoch", ch.active_anchor_epoch);
        c.u64("last_confirm_tick", ch.last_confirm_tick);
        c.u8("last_advance_source", ch.last_advance_source as u8);
    });
}
