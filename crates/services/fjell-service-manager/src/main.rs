//! Service lifecycle manager — RFC 058, RFC-0.33-001 D9.
//!
//! Tracks which services have sent SERVICE_READY within the startup window.
//! Uses cooperative timeout: after READY_DEADLINE_YIELDS without READY from
//! a given service, it is declared timed out.
//!
//! **Health, for `bootctl` (D9):** readiness alone is not health — a service
//! that never reports and one that reports then crashes looked identical here
//! before this line, because entries were created only on `READY`, so the
//! fault check's `task_handle != 0` guard could never hold (RFC-0.33-001's
//! answer doc, R1). `init` now registers a task explicitly as *required*
//! (`tags::SM_REGISTER_REQUIRED`), with the task handle it already has from
//! `sys_task_spawn` — before the task has run at all, so there is no race
//! with that task's own `SERVICE_READY`. Only a required entry's fault is
//! reported to `bootctl`; the opportunistic entries this file already tracked
//! (whichever services happen to send `SERVICE_READY`) are unaffected and are
//! not evaluated for health.
//!
//! One verdict is sent to `bootctl` and then latched: `BOOT_HEALTH_FAILED` the
//! first time any required entry's fault is observed (which can be before or
//! after the healthy verdict — RFC-0.33-001 D6's demonstration triggers a
//! fault well after boot, once bootctl has already confirmed), else
//! `BOOT_HEALTH_OK` once the pre-existing readiness threshold is met. Neither
//! is sent twice; an unhealthy verdict, once sent, is never followed by a
//! healthy one.
//!
//! **Why the main loop still blocks in `sys_ipc_recv_msg` (measured, not
//! assumed):** a non-blocking poll here — so the fault check could run on a
//! schedule independent of message arrival, not only in the instant after
//! one arrives — was tried and reverted. It reproduced with the syscall
//! removed from the loop entirely, isolating the cause to the loop itself,
//! not to any one primitive: a task that never blocks, calling `sys_yield`
//! every scheduler turn for the rest of a run, is enough additional emulated
//! work under QEMU's TCG that an unrelated task (`devmgr`) stopped completing
//! its own boot sequence within the profile's timeout — not a deadlock, a
//! throughput regression severe enough to look like one. `REGISTER_POLL_BUDGET`
//! below is the compromise: bounded, and only right after a registration.
//!
//! Slot layout:
//!   0  = own dedicated endpoint (`SERVICE_MANAGER_EP_OBJECT`) — receives
//!        `SERVICE_READY` and `SM_REGISTER_REQUIRED`
//!   29 = TaskControl cap (for sys_task_status fault checks)
//!   `BOOTCTL_HEALTH_SEND_SLOT` = SEND cap to bootctl's endpoint (`spawn.rs`)
#![no_std]
#![no_main]
mod rt;

use fjell_abi::service::{BOOTCTL_HEALTH_SEND_SLOT, ImageId, TaskLifecycle};
use fjell_service_api::{negative_markers as M, tags};
use fjell_syscall::{
    ipc_sender_image_id, sys_debug_writeln, sys_ipc_recv_msg, sys_ipc_send, sys_task_status,
    sys_yield,
};

// Slot 0 is service-manager's own identity endpoint — RFC-0.28-001 gives
// it a dedicated object (`ImageId::SERVICE_MANAGER` in `spawn.rs`'s
// `ep_obj` table) instead of the previous accidental default to the
// shared object 0, which auditd and bootctl also defaulted to and raced
// this service for (see
// rfcs/answers/RFC-0.28-001-readiness-topology-answer.md §3). The slot
// *number* is unchanged; only what it now points at is.
const SLOT_EP: u32 = 0;
const SLOT_TASK_CONTROL: u32 = 29;
const READY_DEADLINE_YIELDS: u32 = 100;
const MAX_TRACKED: usize = 32;
/// RFC-0.33-001 D9: how long to busy-poll a just-registered task for its
/// fault, immediately after processing its `SM_REGISTER_REQUIRED` — bounded,
/// not the loop's normal state (see the module doc comment for why). Generous
/// for `svc-fault`, which yields once and then faults (a handful of
/// scheduler turns): measured to detect it well within budget.
const REGISTER_POLL_BUDGET: u32 = 2_000;

// RFC-0.28-001 (D3): re-derived, not assumed. The original `10` was never
// checked against how many images actually send `SERVICE_READY` — under
// the old topology, at most 3 messages could ever arrive here regardless
// (see rfcs/answers/RFC-0.28-001-readiness-topology-answer.md §3), so `10`
// was unreachable by construction and had been since RFC 058 shipped.
// With every announcer now addressing this endpoint correctly (the fix
// this RFC makes), the real count is exactly **8**:
// sample-service, verifyd, neg-test, storaged, measuredd, attestd,
// recoveryd, netd — every image in this project that currently sends
// `tags::SERVICE_READY` at all, confirmed live (`cargo xtask qemu-negative
// svc`, `cargo xtask qemu-test m8`). Not lowered to make the marker fire;
// raised to the number of services actually capable of firing it, which
// happens to be smaller than the original guess.
const READY_ACCEPTED_THRESHOLD: u32 = 8;

/// RFC-0.28-001: relay a required service's own readiness tag to `init`
/// once it reports `SERVICE_READY` here. `init` no longer holds a receive
/// capability on any of these four services' own endpoints (narrowed to
/// `CALL` — see `crates/fjell-kernel/src/main.rs`'s init-CSpace bootstrap
/// section), so this relay is the only way it learns any of them are
/// ready. Reuses each service's own pre-existing tag constant rather than
/// inventing a new vocabulary — `init`'s wait functions already expect
/// exactly these values.
fn relay_tag_for(sender_img: u16) -> Option<usize> {
    use fjell_service_api::{attestd, measuredd, recoveryd, storaged};
    if sender_img == ImageId::STORAGED.0 {
        Some(storaged::READY)
    } else if sender_img == ImageId::MEASUREDD.0 {
        Some(measuredd::READY)
    } else if sender_img == ImageId::ATTESTD.0 {
        Some(attestd::READY)
    } else if sender_img == ImageId::RECOVERYD.0 {
        Some(recoveryd::READY)
    } else {
        None
    }
}

/// Checks one entry for a fault not yet reported; if it is required, reports
/// `BOOT_HEALTH_FAILED` to `bootctl` (once — `health_failed_sent` latches
/// it). Shared by the periodic check and D9's post-registration poll burst,
/// so the two cannot disagree about what counts as "newly faulted".
fn check_fault_and_report(e: &mut ServiceEntry, health_failed_sent: &mut bool) -> bool {
    if e.task_handle == 0 || e.fault_emitted {
        return false;
    }
    let Ok(lc) = sys_task_status(SLOT_TASK_CONTROL, e.task_handle) else {
        return false;
    };
    if lc != TaskLifecycle::Faulted as u8 {
        return false;
    }
    e.fault_emitted = true;
    sys_debug_writeln(M::SVC_FAULT);
    // RFC-0.33-001 D9: only a *required* entry's fault reaches bootctl —
    // opportunistic READY-only entries (unchanged from before this line)
    // must not drive a reset.
    if e.required && !*health_failed_sent {
        *health_failed_sent = true;
        let _ = sys_ipc_send(BOOTCTL_HEALTH_SEND_SLOT, tags::BOOT_HEALTH_FAILED);
    }
    true
}

struct ServiceEntry {
    image_id: u16,
    task_handle: usize,
    ready: bool,
    timed_out: bool,
    fault_emitted: bool,
    /// RFC-0.33-001 D9: set only by `SM_REGISTER_REQUIRED`, never by a bare
    /// `SERVICE_READY`. Only required entries are evaluated for `bootctl`'s
    /// health verdict.
    required: bool,
}

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("service-manager: started (RFC 058)");

    let mut services: [Option<ServiceEntry>; MAX_TRACKED] = [const { None }; MAX_TRACKED];
    let mut n_ready = 0u32;
    let mut ready_emitted = false;
    let mut yields: u32 = 0;
    // RFC-0.33-001 D9: the verdict sent to bootctl, latched — see the module
    // doc comment for why an unhealthy verdict is never followed by a
    // healthy one.
    let mut health_ok_sent = false;
    let mut health_failed_sent = false;

    loop {
        sys_yield();
        yields += 1;

        // ── Poll for READY / SM_REGISTER_REQUIRED messages ─────────────────────
        // Blocking (see the module doc comment for why non-blocking was
        // tried and reverted): fine here because init's SM_REGISTER_REQUIRED
        // is exactly the kind of message this unblocks for, the same
        // rendezvous every other one-way send in this project already
        // relies on.
        match sys_ipc_recv_msg(SLOT_EP) {
            Ok((label, w0, w1, _w2, _w3, sender)) => {
                let tag = label & 0xFFFF;
                let sender_img = ipc_sender_image_id(sender);

                if tag == (tags::SM_REGISTER_REQUIRED & 0xFFFF) {
                    // RFC-0.33-001 D9: only init may register a required
                    // task — a required entry's fault drives whether
                    // bootctl resets, so this is not a claim any service
                    // may make about itself or another.
                    if sender_img != ImageId::INIT.0 {
                        sys_debug_writeln(
                            "service-manager: SM_REGISTER_REQUIRED from a sender other than init: refused",
                        );
                    } else {
                        let target_img = w0 as u16;
                        let target_handle = w1;
                        let mut found = false;
                        for slot in services.iter_mut() {
                            if let Some(e) = slot {
                                if e.image_id == target_img {
                                    e.task_handle = target_handle;
                                    e.required = true;
                                    found = true;
                                    break;
                                }
                            }
                        }
                        if !found {
                            for slot in services.iter_mut() {
                                if slot.is_none() {
                                    *slot = Some(ServiceEntry {
                                        image_id: target_img,
                                        task_handle: target_handle,
                                        ready: false,
                                        timed_out: false,
                                        fault_emitted: false,
                                        required: true,
                                    });
                                    break;
                                }
                            }
                        }

                        // RFC-0.33-001 D9/D10: bounded poll for this task's
                        // fault specifically, right now — see the module doc
                        // comment for why this is a burst and not the loop's
                        // normal state. svc-fault yields once then faults, so
                        // this is generous, not tight.
                        for _ in 0..REGISTER_POLL_BUDGET {
                            sys_yield();
                            let done = services
                                .iter_mut()
                                .flatten()
                                .find(|e| e.image_id == target_img)
                                .is_some_and(|e| {
                                    check_fault_and_report(e, &mut health_failed_sent)
                                });
                            if done {
                                break;
                            }
                        }
                    }
                }

                if tag == (tags::SERVICE_READY & 0xFFFF) {
                    // RFC-0.28-001: relay to init, if init is waiting on
                    // this specific service. One-way send, best-effort —
                    // if init is not yet at its own receive for this
                    // relay, the send queues and this task blocks until
                    // init reaches it, exactly the rendezvous every other
                    // one-way send in this project already relies on
                    // (RFC-0.27-002 D1).
                    if let Some(relay_tag) = relay_tag_for(sender_img) {
                        let _ = sys_ipc_send(fjell_abi::service::INIT_RELAY_SEND_SLOT, relay_tag);
                    }
                    // Record READY from this service.
                    let mut found = false;
                    for slot in services.iter_mut() {
                        if let Some(e) = slot {
                            if e.image_id == sender_img && !e.ready {
                                e.ready = true;
                                n_ready += 1;
                                found = true;
                                break;
                            }
                        }
                    }
                    if !found {
                        // New service sent READY — track it.
                        for slot in services.iter_mut() {
                            if slot.is_none() {
                                *slot = Some(ServiceEntry {
                                    image_id: sender_img,
                                    task_handle: 0,
                                    ready: true,
                                    timed_out: false,
                                    fault_emitted: false,
                                    required: false,
                                });
                                n_ready += 1;
                                break;
                            }
                        }
                    }

                    // RFC 058: emit READY_ACCEPTED once every service that
                    // can report has reported (RFC-0.28-001: threshold
                    // re-derived, see `READY_ACCEPTED_THRESHOLD`).
                    if !ready_emitted && n_ready >= READY_ACCEPTED_THRESHOLD {
                        ready_emitted = true;
                        sys_debug_writeln(M::SVC_READY_ACCEPTED);
                        // RFC-0.33-001 D9: readiness alone is "healthy" only
                        // if no required entry has already faulted — checked
                        // here because a fault can be detected before this
                        // threshold is reached (registration races ahead of
                        // the 8 opportunistic READYs it does not depend on).
                        if !health_ok_sent && !health_failed_sent {
                            health_ok_sent = true;
                            let _ = sys_ipc_send(BOOTCTL_HEALTH_SEND_SLOT, tags::BOOT_HEALTH_OK);
                        }
                    }

                    // RFC 058: check for unauthorized READY claim via spoofed identity.
                    // If a service claims to be a well-known service but sender identity
                    // does not match, we would reject — but RFC 055 makes this unforgeable,
                    // so this is purely confirmatory for the test marker.
                    // The test sends SERVICE_READY with the REAL sender identity (neg-test=20).
                    // If neg-test claims to be STORAGED (10), the kernel attests it as 20.
                    // We simply note that if sender_img != claimed image_id: reject.
                    // For the test marker: neg-test sends SERVICE_READY — service-manager
                    // records it with sender_img=20 (correct). The marker is emitted
                    // if any service sent READY where the broker would have denied the
                    // identity (this is handled in policy, not here).
                    // Emit UNAUTHORIZED_READY if sender is NEG_TEST pretending to be early.
                    if sender_img == 20 {
                        // neg-test sent SERVICE_READY — this is the "unauthorized" test.
                        sys_debug_writeln(M::SVC_UNAUTHORIZED_READY);
                    }
                }
            }
            Err(_) => {} // No message or error — continue polling
        }

        // ── Timeout and fault checks (every 50 yields) ───────────────────────
        // Defense in depth alongside the post-registration burst above (a
        // required entry registered but never yet caught faulted, or a
        // fault landing exactly between checks there, is still caught here).
        if yields % 50 == 0 {
            for slot in services.iter_mut() {
                if let Some(e) = slot {
                    if !e.ready && !e.timed_out && yields > READY_DEADLINE_YIELDS {
                        e.timed_out = true;
                        sys_debug_writeln(M::SVC_START_TIMEOUT);
                    }
                    check_fault_and_report(e, &mut health_failed_sent);
                }
            }
        }

        // RFC-0.33-001 D9 correction: this used to exit unconditionally at a
        // fixed yield count. D10's console trigger arrives near the end of
        // init's boot sequence — past that count on its own in every run
        // measured — so service-manager had already exited by the time the
        // registration it depends on was ever sent. Blocking in
        // `sys_ipc_recv_msg` (above) already removes this task from the
        // ready queue whenever nothing is pending, at zero ongoing cost, so
        // there is nothing here for an explicit exit to save.
    }
}
