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
//!
//! RFC-0.37-001 (closes E-078, E-083): `CAP_TOOLS_EP` (slot 1) is installed
//! now — `fleetd` is this service's first real caller, over a console-byte
//! trigger (RFC-v0.8-005's conversation, made real within one node rather
//! than over the network). The tag mismatch E-083 found
//! (the head query addressing numbers `measuredd` never answered,
//! two of which the shared API had already assigned to `recoveryd`) is
//! fixed.
//!
//! RFC-0.38-001 (closes E-085, E-086): `measuredd`'s `HEAD_REPLY` carries the
//! chain digest whole (four words), and the head query reads all four.
//!
//! RFC-0.39-002: `CAP_MEASUREDD_EP` (slot 4) is **installed**, as a `CALL`
//! capability (the head query is a call, not a send and a receive), and
//! `measurement_head_available` reads `true` when the query succeeds. It is not
//! just "the flag turned on": the bundle also carries the chain's composition --
//! how many events it holds and how many of them arrived by the path where the
//! kernel produced the digest -- because a signed bundle that reported a
//! measurement root while withholding that three of the chain's four events are
//! `init`'s literal `0xAB` (E-095) would be a misleading artefact. **This does not
//! make attestation real** (the signed attestation record is independent
//! constants, E-094; `last_attestation_available` stays `false`) **and it does not
//! make the chain fully measured** (three of four events are placeholders, E-095).
#![no_std]
#![no_main]
mod rt;

use fjell_cap::CapHandle;
use fjell_diag_format::{BundleBuilder, MeasurementReport};
use fjell_measure_format::Digest32;
use fjell_service_api::diagnosticsd as proto;
use fjell_service_api::{measuredd, sxt};
use fjell_syscall::{sys_debug_write, sys_debug_writeln, sys_exit, sys_yield};
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

// RFC-0.37-001 R2 (closes E-083): this used to hand-copy five of
// `secure-transportd`'s own `SxtTag` values under local `SXT_*` names, and
// separately declare `MEASUREDD_HEAD_QUERY`/`MEASUREDD_HEAD_REPLY` as
// `0x320`/`0x321` — numbers `fjell-measuredd` has never answered (its real
// `GET_HEAD`/`HEAD_REPLY` arm is `0x303`/`0x304`) and that the shared API had
// already assigned to `recoveryd::READY`/`LIST_SNAPSHOTS`. Demonstrated
// failing first (see the review request): with `CAP_MEASUREDD_EP`
// temporarily installed against the real `measuredd` and these old tags
// still in place, `query_measurement_head` got no reply and the flag stayed
// `false`. Both now read `fjell_service_api::{measuredd, sxt}` (imported
// above) — the single declaration `secure-transportd`'s own `SxtTag` enum
// also reads (`channel.rs`) and `measuredd`'s own dispatch arm already
// used, not a second hand-kept copy.

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

/// What `measuredd` reported, as one snapshot: the head, how many events it had
/// appended, and how many of those arrived by `APPEND_IMAGE_SET`.
///
/// RFC-0.36-001 (closes E-076): the result is `Some` only when real replies were
/// received -- the code that obtains the value is the only code that may say so.
/// A call error, an unexpected tag, or an incoherent answer is `None`, never a
/// zero digest that `build_bundle` could forward as if it were data.
///
/// RFC-0.37-001 (closes E-083): the tags are the shared, real
/// `measuredd::{GET_HEAD, HEAD_REPLY, GET_HEAD_STATS, HEAD_STATS_REPLY}`.
/// RFC-0.38-001 R4 (closes E-085): `measuredd` answers with `sys_ipc_reply`, which
/// needs the reply edge only an `IpcCall` creates, so every query here is a call
/// and the slot needs `CALL`.
///
/// RFC-0.39-002: the head and the counts come from **two** calls, and an append
/// between them would pair a head with the wrong counts. So the statistics are
/// read before and after the head and must be identical; if they are not, the
/// chain moved while it was being read and the query tries again, a few times,
/// then reports `None` rather than a snapshot that never existed.
fn query_measurement() -> Option<MeasurementReport> {
    fn stats() -> Option<(u64, u64)> {
        let (label, w) =
            fjell_syscall::sys_ipc_call4(CAP_MEASUREDD_EP.0, measuredd::GET_HEAD_STATS, [0; 4])
                .ok()?;
        // w[0] = latest_seq (events appended), w[3] = events kernel-digested.
        (label == measuredd::HEAD_STATS_REPLY).then_some((w[0] as u64, w[3] as u64))
    }
    for _attempt in 0..4 {
        let before = stats()?;
        let (label, words) =
            fjell_syscall::sys_ipc_call4(CAP_MEASUREDD_EP.0, measuredd::GET_HEAD, [0; 4]).ok()?;
        if label != measuredd::HEAD_REPLY {
            return None;
        }
        if stats()? != before {
            continue; // the chain moved under us; read it again
        }
        let (events, kernel_digested) = before;
        let events = u32::try_from(events).ok()?;
        let kernel_digested = u32::try_from(kernel_digested).ok()?;
        return MeasurementReport::new(
            Digest32(measuredd::words_to_digest(words)),
            events,
            kernel_digested,
        )
        .ok();
    }
    None
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
    let builder = BundleBuilder::new(*b"diag0001", tick, provider_id, query_measurement());
    builder.finalise()
}

// ── Push via secure-transportd ────────────────────────────────────────────────

fn push_bundle(_bundle: &fjell_diag_format::bundle::DiagnosticBundle) -> bool {
    send_tag(CAP_SXT_EP, sxt::OPEN_CHANNEL);
    let Ok((reply, w0, _)) = recv_msg(CAP_SXT_EP) else {
        return false;
    };
    if reply != sxt::OPENED {
        return false;
    }
    let _channel_id = w0;
    send_tag(CAP_SXT_EP, sxt::DIAG_PUSH);
    let ack = recv_msg(CAP_SXT_EP).map(|(t, _, _)| t).unwrap_or(0);
    send_tag(CAP_SXT_EP, sxt::CLOSE);
    ack == sxt::DIAG_ACK
}

/// Print `n` in decimal (no allocation; `sys_debug_write` takes a `&str`).
fn print_dec(mut n: u64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    if let Ok(s) = core::str::from_utf8(&buf[i..]) {
        sys_debug_write(s);
    }
}

fn print_count(name: &str, value: u32) {
    sys_debug_write("diagnosticsd: ");
    sys_debug_write(name);
    sys_debug_write("=");
    print_dec(value as u64);
    sys_debug_writeln("");
}

/// RFC-0.37-001 R6: one line per flag, at the deciding service's own
/// output — `diagnosticsd: measurement_head_available=false`, and so on for
/// each of the four. The tier asserts these, not just that a bundle exists.
fn print_bool(name: &str, value: bool) {
    sys_debug_write("diagnosticsd: ");
    sys_debug_write(name);
    sys_debug_write("=");
    sys_debug_writeln(if value { "true" } else { "false" });
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

    // RFC-0.37-001 R4 (closes E-078): `CAP_TOOLS_EP` (slot 1) is installed
    // now — `fleetd` is its first real caller. The error path below is no
    // longer the expected, unchanging per-iteration condition RFC-0.35-002
    // R5 described (that was true only while this slot stayed uninstalled);
    // it is now genuine defensive handling for a recv failure that should
    // not happen once the capability is correctly installed, kept rather
    // than removed on the same "yield and keep listening, not exit"
    // principle — a transient failure should not take the service down.
    let mut recv_error_disclosed = false;
    loop {
        let (tag, w0) = match recv_msg(CAP_TOOLS_EP) {
            Err(()) => {
                if !recv_error_disclosed {
                    sys_debug_writeln(
                        "diagnosticsd: CAP_TOOLS_EP recv error -- yielding and listening, \
                         not exiting; this will recur every iteration and is only \
                         disclosed once",
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
            // RFC-0.37-001 R6: the deciding service's own output, not only
            // the instigator's (`fleetd`'s) — the tier asserts flags, not
            // merely "a bundle was built", so each flag's value is printed
            // by name here, the only place that knows why each is what it
            // is.
            print_bool(
                "measurement_head_available",
                bundle.measurement_head_available,
            );
            // RFC-0.39-002 R5: the head itself, read back out of the BUNDLE (what
            // the encoder will sign), so a tier can compare it with the value the
            // host derives. Only when available -- an unavailable head is zero by
            // construction and printing it would be noise.
            if bundle.measurement_head_available {
                let mut hex = [0u8; 64];
                sys_debug_write("diagnosticsd: measurement_head sha256:");
                sys_debug_writeln(bundle.measurement_head.to_hex(&mut hex));
            }
            print_count("measurement_event_count", bundle.measurement_event_count);
            print_count(
                "measurement_events_kernel_digested",
                bundle.measurement_events_kernel_digested,
            );
            print_bool(
                "keyring_anchor_epoch_available",
                bundle.keyring_anchor_epoch_available,
            );
            print_bool(
                "last_attestation_available",
                bundle.last_attestation_available,
            );
            print_bool("audit_available", bundle.audit_available);
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
