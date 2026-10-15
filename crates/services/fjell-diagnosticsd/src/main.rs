//! diagnosticsd — Diagnostic bundle builder and remote push service.
//!
//! v0.4.0: assembles a `DiagnosticBundle` via `fjell-diag-format` and
//! pushes it to a `secure-transportd` Diagnostics channel on operator
//! request (RFC v0.4-005).  Push is operator-initiated only.
//!
//! RFC-0.35-002 (closes E-068): this used to also query `auditd` for
//! recent audit records. `fjell-auditd` never implemented that protocol,
//! so the wait for its reply could not terminate — the first
//! operator-triggered bundle hung the service. Retracted, not stubbed:
//! every bundle's audit section states itself unavailable
//! (`DiagnosticBundle::audit_available`) rather than silently containing
//! nothing. See `rfcs/ERRATA.md`'s E-068 entry for what implementing it
//! for real requires.
#![no_std]
#![no_main]
mod rt;

use fjell_cap::CapHandle;
use fjell_diag_format::BundleBuilder;
use fjell_measure_format::Digest32;
use fjell_service_api::diagnosticsd as proto;
use fjell_syscall::{sys_debug_writeln, sys_exit, sys_yield};
use fjell_trust_provider::ids::TrustProviderId;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    sys_debug_writeln("diagnosticsd: panic");
    sys_exit(1);
}

// ── CSpace layout ─────────────────────────────────────────────────────────────
//
// RFC-0.35-002 (closes E-068): slot 3 — the endpoint to `auditd` the
// retracted audit-query sub-protocol addressed — is gone. Least privilege:
// a slot for a protocol nobody implements grants standing authority this
// service does not exercise (RFC-0.35-002's own ruling, reason 3).
//
// RFC-0.36-001 (closes E-075): `CAP_SMGR_EP` (slot 0) is gone, not reused —
// it was this service's own identity endpoint, not a path to
// `service-manager`, so the readiness send over it reached whichever
// service received there instead (see E-075). Readiness now goes out via
// `fjell_service_api::ready::SERVICE_READY_SEND_SLOT`, the slot every
// correctly-wired service uses, which needs no local constant. The
// remaining slots keep their original numbers: `capability_topology`'s
// waivers and E-076/E-078's own text cite them by number.
const CAP_TOOLS_EP: CapHandle = CapHandle(1);
const CAP_SXT_EP: CapHandle = CapHandle(2);
const CAP_MEASUREDD_EP: CapHandle = CapHandle(4);

// SXT tags mirrored from secure-transportd.
const SXT_OPEN_CHANNEL: usize = 0x0100;
const SXT_OPENED: usize = 0x0101;
const SXT_DIAG_PUSH: usize = 0x0104;
const SXT_DIAG_ACK: usize = 0x0105;
const SXT_CLOSE: usize = 0x0109;

// measuredd head-query protocol.
const MEASUREDD_HEAD_QUERY: usize = 0x320;
const MEASUREDD_HEAD_REPLY: usize = 0x321;

// ── IPC helpers ───────────────────────────────────────────────────────────────

/// RFC-0.34-004 §C (closes E-034): `send_tag` no longer takes a data word.
/// It never carried one — `IpcSend`'s `build_msg`
/// (`crates/fjell-kernel/src/cap/syscall.rs`) only copies `tag.words` words,
/// taken from bits 16-23 of the packed tag, and this call never set them —
/// so removing the parameter changes no runtime behaviour. The
/// `MAX_QUERY_RECORDS`/channel-id/max-count values this file's call sites
/// used to pass were already inert.
///
/// **Updated 2026-10-14 (RFC-0.35-002 review).** This paragraph used to say,
/// in the present tense, that `fjell-auditd` *never implements the
/// `AUDIT_QUERY` protocol* and that the gap was *"not fixed here"*. There is
/// no `AUDIT_QUERY` any more — RFC-0.35-002 retracted the sub-protocol and
/// deleted its only caller — so the sentence described a protocol the tree
/// no longer has. What remains true: `secure-transportd`'s receive loop
/// serves one endpoint and never this service's, which is E-069's closure
/// and `capability-topology`'s waiver for slot 2, not a pending fix.
fn send_tag(ep: CapHandle, tag: usize) {
    let _ = fjell_syscall::sys_ipc_send(ep.0, tag);
}

/// `Err(())` on a recv error, kept distinct from `Ok` so a caller that needs
/// to tell "capability can't receive" apart from "received an unrecognised
/// tag" (RFC-0.35-002 R5) is not forced to overload a sentinel value to do
/// it — `query_measurement_head`/`push_bundle` don't need the distinction
/// (either one is just "no usable reply") and fold it away themselves.
fn recv_msg(ep: CapHandle) -> Result<(usize, usize, usize), ()> {
    // RFC-0.28-002: was a hand-rolled `IpcRecv` asm block; `sys_ipc_recv_msg`
    // is a correct superset (`w2`/`w3`/sender discarded, not needed here).
    match fjell_syscall::sys_ipc_recv_msg(ep.0) {
        Ok((t, w0, w1, _w2, _w3, _sender)) => Ok((t, w0, w1)),
        Err(_) => {
            // RFC-0.35-001 (closes E-072): a recv error must not be a tight
            // loop, independent of which slot caused it or which caller is
            // asking — yielding here, unconditionally, covers every call
            // site in this file, not just the main loop's.
            sys_yield();
            Err(())
        }
    }
}

// ── Measurement head query ────────────────────────────────────────────────────

/// RFC-0.36-001 (closes E-076): the second element is `true` only when a
/// real `MEASUREDD_HEAD_REPLY` was received — the code that obtains the
/// value is the only code that may say so. A recv error or an unexpected
/// tag now returns `(zero digest, false)` instead of a zero digest alone,
/// so `build_bundle` cannot forward a failure as if it were data.
fn query_measurement_head() -> (Digest32, bool) {
    send_tag(CAP_MEASUREDD_EP, MEASUREDD_HEAD_QUERY);
    let Ok((tag, w0_lo, w0_hi)) = recv_msg(CAP_MEASUREDD_EP) else {
        return (Digest32([0u8; 32]), false);
    };
    if tag != MEASUREDD_HEAD_REPLY {
        return (Digest32([0u8; 32]), false);
    }
    let mut d = [0u8; 32];
    d[..8].copy_from_slice(&(w0_lo as u64).to_le_bytes());
    d[8..16].copy_from_slice(&(w0_hi as u64).to_le_bytes());
    (Digest32(d), true)
}

// ── Bundle assembly ───────────────────────────────────────────────────────────

/// RFC-0.35-002 (closes E-068): this used to also query `auditd` here, via
/// a protocol nothing implemented — the wait for a reply that never came
/// could not terminate (`collect_audit_events`, deleted). Retracted, not
/// stubbed: `BundleBuilder::new` leaves `audit_available` at its default
/// `false`, so every bundle states its audit section unavailable rather
/// than silently reporting zero records.
///
/// RFC-0.36-001 (closes E-079): this used to also pass a literal zero as
/// `last_attestation` (never queried) and call `add_intent` with literal
/// arguments claiming an update staging was confirmed two seconds ago,
/// whether or not anything of the kind happened. Both are gone:
/// `BundleBuilder::new` has no parameter to receive a fabricated
/// attestation digest any more, and no call here invents an intent. A
/// bundle with no intents is honest; one with an invented one was not.
fn build_bundle(
    provider_id: TrustProviderId,
    tick: u64,
) -> fjell_diag_format::bundle::DiagnosticBundle {
    let (measurement_head, measurement_head_available) = query_measurement_head();
    let builder = BundleBuilder::new(
        *b"diag0001",
        tick,
        provider_id,
        measurement_head,
        measurement_head_available,
    );
    builder.finalise()
}

// ── Push via secure-transportd ────────────────────────────────────────────────

fn push_bundle(_bundle: &fjell_diag_format::bundle::DiagnosticBundle) -> bool {
    send_tag(CAP_SXT_EP, SXT_OPEN_CHANNEL);
    let Ok((reply, w0, _)) = recv_msg(CAP_SXT_EP) else {
        return false;
    };
    if reply != SXT_OPENED {
        return false;
    }
    let _channel_id = w0;
    send_tag(CAP_SXT_EP, SXT_DIAG_PUSH);
    let ack = recv_msg(CAP_SXT_EP).map(|(t, _, _)| t).unwrap_or(0);
    send_tag(CAP_SXT_EP, SXT_CLOSE);
    ack == SXT_DIAG_ACK
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("diagnosticsd: starting");
    const PROVIDER_ID: TrustProviderId = TrustProviderId(1);

    // RFC-0.36-001 (closes E-075): used to send `proto::READY` on
    // `CAP_SMGR_EP` (slot 0), this service's own identity endpoint, so the
    // tag reached whichever service received there, never
    // `service-manager`. Now sends the generic readiness tag on the
    // uniformly-installed slot every other correctly-wired service uses,
    // matching e.g. `fjell-measuredd`'s `send_ready`.
    let _ = fjell_syscall::sys_ipc_send(
        fjell_service_api::ready::SERVICE_READY_SEND_SLOT,
        fjell_service_api::tags::SERVICE_READY,
    );
    sys_debug_writeln("diagnosticsd: ready");

    // RFC-0.35-002 R5: `CAP_TOOLS_EP` (slot 1) is declared but not installed
    // (RFC-0.35-001 R5 waived it to this line; this line's own non-goals
    // forbid installing it too — "finally clean" does not mean "finally
    // patched over"), so `recv_msg` errors here on every iteration for the
    // rest of every boot, by design, for as long as that stands. Disclosed
    // once, not once per iteration: the condition does not change between
    // iterations, and a line printed on every yield would flood the
    // console and tell an operator nothing the first line did not already
    // say. Chosen behaviour: yield and keep listening, not exit — a future
    // line installing this slot should not require this service to be
    // respawned to notice — and not block, since there is no capability to
    // block on.
    let mut recv_error_disclosed = false;
    loop {
        let (tag, w0) = match recv_msg(CAP_TOOLS_EP) {
            Err(()) => {
                if !recv_error_disclosed {
                    sys_debug_writeln(
                        "diagnosticsd: CAP_TOOLS_EP cannot receive (capability not \
                         installed) -- yielding and listening, not exiting; this will \
                         recur every iteration and is only disclosed once",
                    );
                    recv_error_disclosed = true;
                }
                continue;
            }
            Ok((tag, w0, _w1)) => (tag, w0),
        };
        if tag == proto::BUILD_BUNDLE {
            let bundle = build_bundle(PROVIDER_ID, w0 as u64);
            sys_debug_writeln("diagnosticsd: bundle built");
            let _ = bundle.audit_event_count;
            send_tag(CAP_TOOLS_EP, proto::BUNDLE_READY);
        } else if tag == proto::PUSH {
            let bundle = build_bundle(PROVIDER_ID, w0 as u64);
            let ok = push_bundle(&bundle);
            let reply = if ok {
                proto::PUSH_ACK
            } else {
                proto::PUSH_FAULT
            };
            send_tag(CAP_TOOLS_EP, reply);
            sys_debug_writeln(if ok {
                "diagnosticsd: bundle pushed"
            } else {
                "diagnosticsd: push failed"
            });
        } else {
            // RFC-0.35-002 R5: a real message this loop does not recognise
            // — distinct from a recv error above — is disclosed every time
            // it happens. Unlike the recv error, this is not an expected,
            // unchanging condition: it is an actual message arriving, rate
            // limited by whatever is sending it, not by this loop.
            sys_debug_writeln("diagnosticsd: unexpected tag on CAP_TOOLS_EP");
        }
    }
}
