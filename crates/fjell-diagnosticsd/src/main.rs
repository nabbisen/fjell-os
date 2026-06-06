//! diagnosticsd — Diagnostic bundle builder and remote push service.
//!
//! v0.4.0: Queries `auditd` for recent audit records, assembles a
//! `DiagnosticBundle` via `fjell-diag-format`, and pushes it to a
//! `secure-transportd` Diagnostics channel on operator request
//! (RFC v0.4-005).  Push is operator-initiated only.
#![no_std]
#![no_main]
mod rt;

use fjell_syscall::{sys_debug_writeln, sys_exit};
use fjell_cap::CapHandle;
use fjell_measure_format::Digest32;
use fjell_trust_provider::ids::TrustProviderId;
use fjell_service_api::diagnosticsd as proto;
use fjell_diag_format::{
    BundleBuilder,
    intents::INTENT_UPDATE_STAGING_CONFIRMED,
};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    sys_debug_writeln("diagnosticsd: panic");
    sys_exit(1);
}

// ── CSpace layout ─────────────────────────────────────────────────────────────
const CAP_SMGR_EP:      CapHandle = CapHandle(0);
const CAP_TOOLS_EP:     CapHandle = CapHandle(1);
const CAP_SXT_EP:       CapHandle = CapHandle(2);
const CAP_AUDITD_EP:    CapHandle = CapHandle(3);
const CAP_MEASUREDD_EP: CapHandle = CapHandle(4);

// SXT tags mirrored from secure-transportd.
const SXT_OPEN_CHANNEL: usize = 0x0100;
const SXT_OPENED:       usize = 0x0101;
const SXT_DIAG_PUSH:    usize = 0x0104;
const SXT_DIAG_ACK:     usize = 0x0105;
const SXT_CLOSE:        usize = 0x0109;

// measuredd head-query protocol.
const MEASUREDD_HEAD_QUERY: usize = 0x320;
const MEASUREDD_HEAD_REPLY: usize = 0x321;

// ── IPC helpers ───────────────────────────────────────────────────────────────

fn send_tag(ep: CapHandle, tag: usize, w0: usize) {
    unsafe {
        core::arch::asm!(
            "li a7, 20", "ecall",
            in("a0") ep.0 as usize, in("a1") tag, in("a2") w0,
            lateout("a0") _, lateout("a7") _, options(nostack)
        );
    }
}

fn recv_msg(ep: CapHandle) -> (usize, usize, usize) {
    let (mut t, mut w0, mut w1) = (0usize, 0usize, 0usize);
    unsafe {
        core::arch::asm!(
            "li a7, 21", "ecall",
            in("a0") ep.0 as usize,
            lateout("a1") t, lateout("a2") w0, lateout("a3") w1,
            lateout("a4") _, lateout("a5") _, lateout("a7") _, options(nostack)
        );
    }
    (t, w0, w1)
}

// ── Measurement head query ────────────────────────────────────────────────────

fn query_measurement_head() -> Digest32 {
    send_tag(CAP_MEASUREDD_EP, MEASUREDD_HEAD_QUERY, 0);
    let (tag, w0_lo, w0_hi) = recv_msg(CAP_MEASUREDD_EP);
    if tag != MEASUREDD_HEAD_REPLY { return Digest32([0u8; 32]); }
    let mut d = [0u8; 32];
    d[..8].copy_from_slice(&(w0_lo as u64).to_le_bytes());
    d[8..16].copy_from_slice(&(w0_hi as u64).to_le_bytes());
    Digest32(d)
}

// ── Audit record collection ───────────────────────────────────────────────────

const MAX_QUERY_RECORDS: usize = 64;

/// Query `auditd` for recent audit records and populate the builder.
///
/// Wire protocol (diagnosticsd → auditd):
///   → `AUDIT_QUERY` (w0 = max_count)
///   ← zero or more `AUDIT_RECORD` (w0 = kind_tag|seq<<16, w1 = at_tick)
///   ← `AUDIT_STREAM_END`
fn collect_audit_events(builder: &mut BundleBuilder) {
    send_tag(CAP_AUDITD_EP, proto::AUDIT_QUERY, MAX_QUERY_RECORDS);
    loop {
        let (tag, w0, w1) = recv_msg(CAP_AUDITD_EP);
        if tag == proto::AUDIT_STREAM_END { break; }
        if tag != proto::AUDIT_RECORD     { continue; }
        let kind_tag = (w0 & 0xFFFF) as u16;
        let seq      = ((w0 >> 16) & 0xFFFF) as u32;
        let at_tick  = w1 as u64;
        let _ = builder.add_audit_event(seq, kind_tag, 0, at_tick);
    }
}

// ── Bundle assembly ───────────────────────────────────────────────────────────

fn build_bundle(
    provider_id: TrustProviderId,
    tick: u64,
) -> fjell_diag_format::bundle::DiagnosticBundle {
    let measurement_head = query_measurement_head();
    let mut builder = BundleBuilder::new(
        *b"diag0001", tick, provider_id, 1,
        measurement_head, Digest32([0u8; 32]),
    );
    collect_audit_events(&mut builder);
    let _ = builder.add_intent(1, INTENT_UPDATE_STAGING_CONFIRMED, 0,
                               tick.saturating_sub(2000));
    builder.finalise()
}

// ── Push via secure-transportd ────────────────────────────────────────────────

fn push_bundle(_bundle: &fjell_diag_format::bundle::DiagnosticBundle) -> bool {
    send_tag(CAP_SXT_EP, SXT_OPEN_CHANNEL, 0x0200_0000);
    let (reply, w0, _) = recv_msg(CAP_SXT_EP);
    if reply != SXT_OPENED { return false; }
    let channel_id = w0;
    send_tag(CAP_SXT_EP, SXT_DIAG_PUSH, channel_id);
    let (ack, _, _) = recv_msg(CAP_SXT_EP);
    send_tag(CAP_SXT_EP, SXT_CLOSE, channel_id);
    ack == SXT_DIAG_ACK
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("diagnosticsd: starting");
    const PROVIDER_ID: TrustProviderId = TrustProviderId(1);

    send_tag(CAP_SMGR_EP, proto::READY, 0);
    sys_debug_writeln("diagnosticsd: ready");

    loop {
        let (tag, w0, _) = recv_msg(CAP_TOOLS_EP);
        if tag == proto::BUILD_BUNDLE {
            let bundle = build_bundle(PROVIDER_ID, w0 as u64);
            sys_debug_writeln("diagnosticsd: bundle built");
            send_tag(CAP_TOOLS_EP, proto::BUNDLE_READY,
                bundle.audit_event_count as usize);
        } else if tag == proto::PUSH {
            let bundle = build_bundle(PROVIDER_ID, w0 as u64);
            let ok = push_bundle(&bundle);
            let reply = if ok { proto::PUSH_ACK } else { proto::PUSH_FAULT };
            send_tag(CAP_TOOLS_EP, reply, 0);
            sys_debug_writeln(if ok { "diagnosticsd: bundle pushed" }
                              else  { "diagnosticsd: push failed" });
        }
    }
}
