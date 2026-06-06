//! summaryd — Measurement and release summary exporter (RFC v0.7-003).
//!
//! Responsibilities:
//!   1. Periodically export `MeasurementSummary` from measuredd.
//!   2. Export `ReleaseSummary` from upgraded counter tables.
//!   3. Sign both summaries via attestd and persist to storaged.
#![no_std]
#![no_main]
mod rt;
use fjell_syscall::{sys_exit, sys_debug_writeln};
use fjell_summary_format::{
    MeasurementSummary, ReleaseSummary, ChannelSummary, AdvanceSource,
    measurement_summary_digest, release_summary_digest,
};
#[allow(unused_imports)] // stub: NodeId used in summary signing in v0.7.x
use fjell_identity_format::NodeId;
use fjell_measure_format::Digest32;

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("summaryd: started (v0.7 summary exporter)");

    let node_id = [0x01u8; 16];

    // Stub MeasurementSummary.
    let mut _ms = MeasurementSummary::new(
        node_id, 0, 0,
        Digest32([0u8; 32]),
        Digest32([0u8; 32]),
    );
    let _ = _ms.add_kind_count(0x01, 1);  // BootEvidenceImported = 1 event
    _ms.summary_digest = measurement_summary_digest(&_ms);
    sys_debug_writeln("summaryd: measurement summary ready");

    // Stub ReleaseSummary.
    let mut _rs = ReleaseSummary::new(node_id, 0);
    let _ = _rs.add_channel(ChannelSummary {
        channel_id:          *b"default\0",
        current_counter:     0,
        min_counter:         0,
        active_anchor_epoch: 0,
        last_confirm_tick:   0,
        last_advance_source: AdvanceSource::Unknown,
    });
    _rs.summary_digest = release_summary_digest(&_rs);
    sys_debug_writeln("summaryd: release summary ready");

    sys_exit(0)
}
