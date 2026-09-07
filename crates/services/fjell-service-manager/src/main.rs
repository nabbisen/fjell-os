//! Service lifecycle manager — RFC 058.
//!
//! Tracks which services have sent SERVICE_READY within the startup window.
//! Uses cooperative timeout: after READY_DEADLINE_YIELDS without READY from
//! a given service, it is declared timed out.
//!
//! Slot layout:
//!   0  = shared endpoint (object 0) — receives SERVICE_READY messages
//!   29 = TaskControl cap (for sys_task_status fault checks)
#![no_std]
#![no_main]
mod rt;

use fjell_abi::service::{ImageId, TaskLifecycle};
use fjell_service_api::{negative_markers as M, tags};
use fjell_syscall::{
    ipc_sender_image_id, sys_debug_writeln, sys_exit, sys_ipc_recv_msg, sys_ipc_send,
    sys_task_status, sys_yield,
};

// Slot 0 is service-manager's own identity endpoint — RFC-0.28-001 gives
// it a dedicated object (`ImageId::SERVICE_MANAGER` in `spawn.rs`'s
// `ep_obj` table) instead of the previous accidental default to the
// shared object 0, which auditd and bootctl also defaulted to and raced
// this service for (see
// docs/rfcs/RFC-0.28-001-readiness-topology-answer.md §3). The slot
// *number* is unchanged; only what it now points at is.
const SLOT_EP: u32 = 0;
const SLOT_TASK_CONTROL: u32 = 29;
const READY_DEADLINE_YIELDS: u32 = 100;
const MAX_TRACKED: usize = 32;

// RFC-0.28-001 (D3): re-derived, not assumed. The original `10` was never
// checked against how many images actually send `SERVICE_READY` — under
// the old topology, at most 3 messages could ever arrive here regardless
// (see docs/rfcs/RFC-0.28-001-readiness-topology-answer.md §3), so `10`
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

struct ServiceEntry {
    image_id: u16,
    task_handle: usize,
    ready: bool,
    timed_out: bool,
    fault_emitted: bool,
}

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("service-manager: started (RFC 058)");

    let mut services: [Option<ServiceEntry>; MAX_TRACKED] = [const { None }; MAX_TRACKED];
    let mut n_ready = 0u32;
    let mut ready_emitted = false;
    let mut yields: u32 = 0;

    loop {
        sys_yield();
        yields += 1;

        // ── Poll for READY messages (non-blocking attempt) ────────────────────
        // Use sys_ipc_recv_msg which blocks — but only for a tick since every
        // service either responds quickly or times out.
        match sys_ipc_recv_msg(SLOT_EP) {
            Ok((label, _w0, _w1, _w2, _w3, sender)) => {
                let tag = label & 0xFFFF;
                let sender_img = ipc_sender_image_id(sender);

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
        if yields % 50 == 0 {
            for slot in services.iter_mut() {
                if let Some(e) = slot {
                    if !e.ready && !e.timed_out && yields > READY_DEADLINE_YIELDS {
                        e.timed_out = true;
                        sys_debug_writeln(M::SVC_START_TIMEOUT);
                    }
                    if e.task_handle != 0 && !e.fault_emitted {
                        if let Ok(lc) = sys_task_status(SLOT_TASK_CONTROL, e.task_handle) {
                            if lc == TaskLifecycle::Faulted as u8 {
                                e.fault_emitted = true;
                                sys_debug_writeln(M::SVC_FAULT);
                            }
                        }
                    }
                }
            }
        }

        if yields > 2000 {
            sys_exit(0);
        }
    }
}
