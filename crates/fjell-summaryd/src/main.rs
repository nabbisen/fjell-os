//! summaryd — Measurement and release summary exporter (RFC v0.7-003, wired RFC-v0.7.2-001).
#![no_std]
#![no_main]
mod rt;
use fjell_syscall::{sys_exit, sys_debug_writeln};
use fjell_summary_format::{
    MeasurementSummary, ReleaseSummary, ChannelSummary, AdvanceSource,
    SummaryError,
    measurement_summary_digest, release_summary_digest,
};
use fjell_measure_format::Digest32;

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("summaryd: started (v0.7 summary exporter)");

    let node_id = [0x01u8; 16];

    // ── MeasurementSummary ────────────────────────────────────────────────────
    // RFC-v0.7.2-001: add_kind_count now returns Result with SummaryError.
    // Duplicate kinds are rejected (closes C-M-04).

    let mut ms = MeasurementSummary::new(
        node_id, 0, 0,
        Digest32([0u8; 32]),
        Digest32([0u8; 32]),
    );

    // Add measurement kind counts (boot evidence + policy load).
    match ms.add_kind_count(0x10, 1) {  // PlatformProfileLoaded = 1 event
        Ok(()) => {},
        Err(SummaryError::DuplicateKind) => {
            sys_debug_writeln("summaryd: ERROR duplicate kind");
            sys_exit(1);
        }
        Err(_) => {
            sys_debug_writeln("summaryd: ERROR adding kind count");
            sys_exit(1);
        }
    }

    // Verify duplicate is correctly rejected.
    if ms.add_kind_count(0x10, 5) != Err(SummaryError::DuplicateKind) {
        sys_debug_writeln("summaryd: ERROR duplicate kind not rejected");
        sys_exit(1);
    }

    ms.summary_digest = measurement_summary_digest(&ms);
    if ms.summary_digest.0 == [0u8; 32] {
        sys_debug_writeln("summaryd: ERROR measurement digest is zero");
        sys_exit(1);
    }

    sys_debug_writeln("summaryd: emitted measurement_summary head_seq=0");

    // ── ReleaseSummary ────────────────────────────────────────────────────────

    let mut rs = ReleaseSummary::new(node_id, 0);

    let ch = ChannelSummary {
        channel_id:          *b"default\0",
        current_counter:     0,
        min_counter:         0,
        active_anchor_epoch: 0,
        last_confirm_tick:   0,
        last_advance_source: AdvanceSource::Unknown,
    };

    match rs.add_channel(ch) {
        Ok(()) => {},
        Err(e) => {
            sys_debug_writeln("summaryd: ERROR adding channel");
            let _ = e;
            sys_exit(1);
        }
    }

    // Verify duplicate channel is rejected.
    if rs.add_channel(ch) != Err(SummaryError::DuplicateChannel) {
        sys_debug_writeln("summaryd: ERROR duplicate channel not rejected");
        sys_exit(1);
    }

    rs.summary_digest = release_summary_digest(&rs);
    if rs.summary_digest.0 == [0u8; 32] {
        sys_debug_writeln("summaryd: ERROR release digest is zero");
        sys_exit(1);
    }

    sys_debug_writeln("summaryd: emitted release_summary channels=1");
    sys_debug_writeln("summaryd: release summary ready");
    sys_exit(0)
}
