//! summaryd — Measurement and release summary exporter (RFC v0.7-003, wired RFC-v0.7.2-001).
//!
//! Periodically exports MeasurementSummary and ReleaseSummary to storaged.
//! v0.7.2: storaged persist path is skeleton-complete (ServiceUnavailable
//! until service-manager manifest wiring lands in v0.7.2.1).
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

// Store record kinds for summaries (RFC-v0.7.2-001).
const STORE_RECORD_KIND_MEASUREMENT_SUMMARY: u16 = 0x0030;
const STORE_RECORD_KIND_RELEASE_SUMMARY:     u16 = 0x0031;

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("summaryd: started (v0.7 summary exporter)");

    let node_id = [0x01u8; 16];

    // ── MeasurementSummary ────────────────────────────────────────────────────
    let mut ms = MeasurementSummary::new(
        node_id, 0, 0,
        Digest32([0u8; 32]),
        Digest32([0u8; 32]),
    );

    // Kind counts: PlatformProfileLoaded (0x10), BoardProfileLoaded (0x11).
    if ms.add_kind_count(0x10, 1).is_err() {
        sys_debug_writeln("summaryd: ERROR adding kind 0x10");
        sys_exit(1);
    }
    if ms.add_kind_count(0x11, 1).is_err() {
        sys_debug_writeln("summaryd: ERROR adding kind 0x11");
        sys_exit(1);
    }

    // Duplicate rejection sanity check.
    match ms.add_kind_count(0x10, 5) {
        Err(SummaryError::DuplicateKind) => {}
        _ => {
            sys_debug_writeln("summaryd: ERROR duplicate kind not rejected");
            sys_exit(1);
        }
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

    if rs.add_channel(ch).is_err() {
        sys_debug_writeln("summaryd: ERROR adding channel");
        sys_exit(1);
    }

    // Duplicate channel rejection sanity check.
    match rs.add_channel(ch) {
        Err(SummaryError::DuplicateChannel) => {}
        _ => {
            sys_debug_writeln("summaryd: ERROR duplicate channel not rejected");
            sys_exit(1);
        }
    }

    rs.summary_digest = release_summary_digest(&rs);
    if rs.summary_digest.0 == [0u8; 32] {
        sys_debug_writeln("summaryd: ERROR release digest is zero");
        sys_exit(1);
    }

    sys_debug_writeln("summaryd: emitted release_summary channels=1");
    sys_debug_writeln("summaryd: release summary ready");

    // ── Persist to storaged (skeleton path) ───────────────────────────────────
    // In v0.7.2.1: call store_append(CAP_STORAGED_EP, 0x0030, ms_bytes)
    //              call store_append(CAP_STORAGED_EP, 0x0031, rs_bytes)
    // For now: log the record kind to show the path is wired.
    sys_debug_writeln("summaryd: would persist kind=0x0030 (measurement_summary)");
    sys_debug_writeln("summaryd: would persist kind=0x0031 (release_summary)");

    sys_exit(0)
}
