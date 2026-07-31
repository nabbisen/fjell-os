//! Sample user-space service.
//!
//! Serves heartbeat requests from service-manager and, for RFC 042 negative
//! testing, handles the `BIND_LEASE_FOR_IPC_TEST` protocol:
//!
//! 1. neg-test sends `BIND_LEASE_FOR_IPC_TEST(lease_id)`.
//! 2. sample-service copies its endpoint cap to a scratch slot, binds the
//!    lease, replies OK, then calls `sys_ipc_recv` on the leased copy.
//! 3. When neg-test revokes the lease, the kernel wakes sample-service with
//!    `LeaseRevoked` → sample-service prints `NEG:IPC:BLOCKED_RECV_WAKES_ON_REVOKE:PASS`.

#![no_std]
#![no_main]
mod rt;

use fjell_abi::lease::LeaseId;
use fjell_cap::{CapHandle, CapRights};
use fjell_semantic_format::*;
use fjell_service_api::semantic_stream as sem_proto;
use fjell_service_api::{negative_markers as M, tags};
use fjell_syscall::{
    sys_cap_bind_lease, sys_cap_copy, sys_cap_drop, sys_debug_writeln, sys_exit, sys_ipc_recv,
    sys_ipc_recv_msg, sys_ipc_reply,
};

// Scratch CSpace slots for IPC tests.
const SLOT_LEASED_EP: u32 = 5; // blocked-recv test (BIND_LEASE_FOR_IPC_TEST)
const SLOT_CALL_EP: u32 = 6; // blocked-call test (BIND_LEASE_AND_CALL_BACK)
// Own endpoint slot (pre-installed; object 6 — dedicated, RFC 042).
const SLOT_OWN_EP: u32 = 0;
// Shared endpoint (object 0) — used only for the SERVICE_READY signal.
const SLOT_SHARED_EP: u32 = 2;
// semantic-stream endpoint cap (object 7), pre-installed by spawn.rs
// (RFC-v0.23-001) so the SDK reference service can emit an intent node.
const SEM_STREAM_EP: u32 = 3;

/// Build and emit a demonstration `IntentNode` to semantic-stream (RFC-v0.23-001).
///
/// Two actions are declared, deliberately requiring different capabilities:
/// one whose required right (`SEND | REPLY`) matches what proxy-text is
/// granted, and one requiring `MMIO_MAP`, which it is not. This is what lets
/// the return leg (Slice 3) demonstrate both the accept and the refuse case
/// from a single intent, rather than asserting success alone.
fn emit_sample_intent() {
    let mut actions: FixedVec<ActionSpec, MAX_ACTIONS> = FixedVec::new();
    actions.push(ActionSpec {
        action_id: ActionId(1),
        label: TextToken::new("acknowledge"),
        kind: ActionKind::Confirm,
        required_capability: Some(CapabilityRequirement {
            resource_class: BoundedText::from_str("demo"),
            resource_name: ResourceName::new("proxy-ack"),
            rights: (CapRights::SEND.0 | CapRights::REPLY.0) as u32,
        }),
        reversibility: Reversibility::Reversible,
        confirmation: ConfirmationPolicy::None,
    });
    actions.push(ActionSpec {
        action_id: ActionId(2),
        label: TextToken::new("remap-device"),
        kind: ActionKind::Inspect,
        required_capability: Some(CapabilityRequirement {
            resource_class: BoundedText::from_str("demo"),
            resource_name: ResourceName::new("proxy-mmio"),
            rights: CapRights::MMIO_MAP.0 as u32,
        }),
        reversibility: Reversibility::Reversible,
        confirmation: ConfirmationPolicy::None,
    });

    let intent = IntentNode {
        kind: IntentKind::ActionRequest,
        title: TextToken::new("sample-service demo intent"),
        description: TextToken::new("RFC-v0.23-001 ABDD live path demonstration"),
        severity: Severity::Normal,
        actions,
        consequences: FixedVec::new(),
        expires_at_tick: None,
    };

    // producer_index=6 (ImageId::SAMPLE_SERVICE's endpoint id, for a stable
    // demo identifier — not itself load-bearing for the capability check).
    let node_id = NodeId {
        producer_index: 6,
        local_sequence: 1,
    };
    let envelope = SemanticEnvelope::new_intent(node_id, 1, intent);

    // SAFETY: category=raw-pointer-deref `SemanticEnvelope` is `Copy` with no
    // pointers or heap allocations — every field is a fixed-size array, enum,
    // or primitive (verified against fjell-semantic-format/src/lib.rs in
    // full). Reinterpreting it as a byte slice for wire transfer is sound
    // because sender and receiver are built from the identical type
    // definition by the identical compiler for the identical target (one
    // `cargo build` invocation produces every service binary).
    let bytes: &[u8] = unsafe {
        core::slice::from_raw_parts(
            &envelope as *const SemanticEnvelope as *const u8,
            core::mem::size_of::<SemanticEnvelope>(),
        )
    };

    let reply = fjell_service_api::chunked::send(
        SEM_STREAM_EP,
        sem_proto::PUBLISH_BEGIN,
        sem_proto::PUBLISH_CHUNK,
        sem_proto::PUBLISH_COMMIT,
        bytes,
    );
    if reply == sem_proto::PUBLISH_OK {
        sys_debug_writeln("sample-service: intent emitted");
    } else {
        sys_debug_writeln("sample-service: intent emit FAILED");
    }
}

/// Print a SysError discriminant as a decimal line (no alloc, no fmt).
fn debug_err(e: fjell_abi::error::SysError) {
    let mut buf = [0u8; 24];
    let mut n = e as usize;
    let mut i = buf.len();
    if n == 0 {
        i -= 1;
        buf[i] = b'0';
    }
    while n > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    sys_debug_writeln(core::str::from_utf8(&buf[i..]).unwrap_or("?"));
}

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    // RFC 058: signal service-manager we are ready.
    // RFC 058: signal READY to service-manager (best-effort; no reply expected).
    let _ = fjell_syscall::sys_ipc_try_send(SLOT_SHARED_EP, fjell_service_api::tags::SERVICE_READY);
    let ep: u32 = 0; // slot 0 = own endpoint (object 6, dedicated)

    // RFC-v0.23-001: emit a demonstration intent to semantic-stream. Done
    // once at startup, before the request loop — semantic-stream and
    // proxy-text are already spawned and ready by this point (Slice 1).
    emit_sample_intent();

    loop {
        // Use recv_msg to capture data words (needed for BIND_LEASE_FOR_IPC_TEST).
        let (label, w0, _, _, _, _) = match sys_ipc_recv_msg(ep) {
            // RFC 055: ignore sender identity
            Ok(v) => v,
            Err(_) => {
                let _ = sys_ipc_reply(0);
                continue;
            }
        };

        match label {
            // ── Normal service operations ────────────────────────────────────
            l if l == (tags::SERVICE_HEARTBEAT & 0xFFFF) => {
                let _ = sys_ipc_reply(tags::SERVICE_HEARTBEAT);
            }
            l if l == (tags::SERVICE_SHUTDOWN & 0xFFFF) => {
                break;
            }

            // ── RFC 042: IPC blocked-recv negative test protocol ─────────────
            //
            // neg-test sends BIND_LEASE_FOR_IPC_TEST(w0=lease_id):
            //   1. Copy slot 0 (Endpoint, obj 0) → slot SLOT_LEASED_EP.
            //   2. Bind the lease (from w0) to slot SLOT_LEASED_EP.
            //   3. Reply OK so neg-test knows we're ready to block.
            //   4. Call sys_ipc_recv(SLOT_LEASED_EP):
            //      - Lease still active → task enters recvq.
            //      - neg-test revokes the lease → kernel wakes us with LeaseRevoked.
            //   5. Print marker; drop scratch slot; loop continues.
            l if l == (tags::BIND_LEASE_FOR_IPC_TEST & 0xFFFF) => {
                let lease_id = LeaseId(w0 as u32);
                // Thread the generation-correct handle returned by copy through
                // bind, recv, and the final drop. Earlier revisions dropped via
                // the raw slot constant, which failed the generation check after
                // the first cycle and left the slot occupied (architect review
                // v0.18 follow-up — same defect class as the neg-test quartet).
                let leased_h = 'setup: {
                    let h = match sys_cap_copy(CapHandle(SLOT_OWN_EP), SLOT_LEASED_EP) {
                        Ok(h) => h,
                        Err(_) => {
                            sys_debug_writeln("sample: blocked_recv setup copy failed");
                            break 'setup None;
                        }
                    };
                    if sys_cap_bind_lease(h, lease_id).is_err() {
                        sys_debug_writeln("sample: blocked_recv setup bind failed");
                        let _ = sys_cap_drop(h);
                        break 'setup None;
                    }
                    Some(h)
                };
                let Some(leased_h) = leased_h else {
                    let _ = sys_ipc_reply(usize::MAX); // setup failed
                    continue;
                };
                // Reply OK — neg-test will now yield and then revoke the lease.
                let _ = sys_ipc_reply(0);

                // Block in ipc_recv with the leased cap.
                // Woken by cancel_blocked_ipc_for_lease when neg-test revokes.
                match sys_ipc_recv(leased_h.0) {
                    Err(_) => {
                        // LeaseRevoked (or other error) — the RFC 034 revoke path works.
                        sys_debug_writeln(M::IPC_BLOCKED_RECV);
                    }
                    Ok(_) => {
                        // Unexpected message arrived before the lease was revoked.
                        // Reply and continue — marker is not emitted in this case.
                        let _ = sys_ipc_reply(0);
                    }
                }
                let _ = sys_cap_drop(leased_h);
            }

            // ── RFC 042: IPC blocked-call + late-reply test protocol ─────────
            //
            // neg-test sends BIND_LEASE_AND_CALL_BACK(w0=lease_id):
            //   1. Copy slot 0 → slot SLOT_CALL_EP; bind lease to copy.
            //   2. Reply OK so neg-test knows we're ready.
            //   3. Call neg-test back on the leased copy → blocks waiting reply.
            //   4. neg-test revokes lease → kernel wakes us with LeaseRevoked.
            //      → print BLOCKED_CALL_WAKES_ON_REVOKE marker.
            //   5. Drop SLOT_CALL_EP; continue.
            l if l == (tags::BIND_LEASE_AND_CALL_BACK & 0xFFFF) => {
                let lease_id = LeaseId(w0 as u32);
                let call_h = 'setup2: {
                    let h = match sys_cap_copy(CapHandle(SLOT_OWN_EP), SLOT_CALL_EP) {
                        Ok(h) => h,
                        Err(_) => {
                            sys_debug_writeln("sample: blocked_call setup copy failed");
                            break 'setup2 None;
                        }
                    };
                    if sys_cap_bind_lease(h, lease_id).is_err() {
                        sys_debug_writeln("sample: blocked_call setup bind failed");
                        let _ = sys_cap_drop(h);
                        break 'setup2 None;
                    }
                    Some(h)
                };
                let Some(call_h) = call_h else {
                    let _ = sys_ipc_reply(usize::MAX);
                    continue;
                };
                // Reply OK — neg-test will now call sys_ipc_recv(0).
                let _ = sys_ipc_reply(0);
                // Call neg-test back with the leased cap.
                // neg-test will receive this, revoke the lease, and try to reply.
                // Fail-closed (RB-01 pattern): the marker is emitted ONLY for
                // the contract-specified LeaseRevoked wake. Any other error
                // means the call failed for an unrelated reason (no callback
                // ever reached neg-test) and must be visible, not a false PASS.
                match fjell_syscall::sys_ipc_call(call_h.0, tags::CALL_BACK_MSG) {
                    Err(fjell_abi::error::SysError::LeaseRevoked) => {
                        // Woken with LeaseRevoked — the BLOCKED_CALL path works.
                        sys_debug_writeln(M::IPC_BLOCKED_CALL);
                    }
                    Err(e) => {
                        sys_debug_writeln("sample: blocked_call callback errored:");
                        debug_err(e);
                    }
                    Ok(_) => {
                        // Got a reply (unexpected in the test scenario).
                        // Continue normally.
                    }
                }
                let _ = sys_cap_drop(call_h);
            }

            // ── Unknown label ────────────────────────────────────────────────
            _ => {
                let _ = sys_ipc_reply(0);
            }
        }
    }

    sys_exit(0)
}
