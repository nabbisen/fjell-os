//! Canonical summary digest computations (RFC v0.7-003 §6.1).

use fjell_measure_format::Digest32;
use crate::measurement::{MeasurementSummary, MSUMMARY_SCHEMA_VERSION};
use crate::release::{ReleaseSummary, RSUMMARY_SCHEMA_VERSION};

/// `SHA256("FJELL-MSUMMARY-V1" || schema || source_node_id || ...)`.
pub fn measurement_summary_digest(s: &MeasurementSummary) -> Digest32 {
    let mut buf = [0u8; 512];
    let mut pos = 0usize;

    macro_rules! w_u8  { ($v:expr) => { buf[pos] = $v; pos += 1; }; }
    macro_rules! w_u16 { ($v:expr) => { buf[pos..pos+2].copy_from_slice(&($v as u16).to_le_bytes()); pos += 2; }; }
    macro_rules! w_u32 { ($v:expr) => { buf[pos..pos+4].copy_from_slice(&($v as u32).to_le_bytes()); pos += 4; }; }
    macro_rules! w_u64 { ($v:expr) => { buf[pos..pos+8].copy_from_slice(&($v as u64).to_le_bytes()); pos += 8; }; }
    macro_rules! w_b   { ($b:expr) => { let bb: &[u8] = $b; buf[pos..pos+bb.len()].copy_from_slice(bb); pos += bb.len(); }; }

    w_b!(b"FJELL-MSUMMARY-V1");
    w_u16!(MSUMMARY_SCHEMA_VERSION);
    w_b!(&s.source_node_id);
    w_u64!(s.issued_tick);
    w_u64!(s.head_seq);
    w_b!(&s.head_chain_digest.0);
    w_u8!(s.kind_count);
    for i in 0..s.kind_count as usize {
        w_u8!(s.kind_counts[i].kind);
        w_u32!(s.kind_counts[i].count);
    }
    w_b!(&s.policy_digest.0);

    Digest32::of(&buf[..pos])
}

/// `SHA256("FJELL-RSUMMARY-V1" || schema || source_node_id || ...)`.
pub fn release_summary_digest(s: &ReleaseSummary) -> Digest32 {
    let mut buf = [0u8; 512];
    let mut pos = 0usize;

    macro_rules! w_u8  { ($v:expr) => { buf[pos] = $v; pos += 1; }; }
    macro_rules! w_u16 { ($v:expr) => { buf[pos..pos+2].copy_from_slice(&($v as u16).to_le_bytes()); pos += 2; }; }
    macro_rules! w_u32 { ($v:expr) => { buf[pos..pos+4].copy_from_slice(&($v as u32).to_le_bytes()); pos += 4; }; }
    macro_rules! w_u64 { ($v:expr) => { buf[pos..pos+8].copy_from_slice(&($v as u64).to_le_bytes()); pos += 8; }; }
    macro_rules! w_b   { ($b:expr) => { let bb: &[u8] = $b; buf[pos..pos+bb.len()].copy_from_slice(bb); pos += bb.len(); }; }

    w_b!(b"FJELL-RSUMMARY-V1");
    w_u16!(RSUMMARY_SCHEMA_VERSION);
    w_b!(&s.source_node_id);
    w_u64!(s.issued_tick);
    w_u8!(s.channel_count);
    for i in 0..s.channel_count as usize {
        let ch = &s.channels[i];
        w_b!(&ch.channel_id);
        w_u64!(ch.current_counter);
        w_u64!(ch.min_counter);
        w_u32!(ch.active_anchor_epoch);
        w_u64!(ch.last_confirm_tick);
        w_u8!(ch.last_advance_source as u8);
    }

    Digest32::of(&buf[..pos])
}
