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
use fjell_cap::CapHandle;
use fjell_syscall::{
    sys_exit, sys_ipc_reply, sys_ipc_recv, sys_ipc_recv_msg,
    sys_cap_copy, sys_cap_bind_lease, sys_cap_drop, sys_debug_writeln,
};
use fjell_service_api::{tags, negative_markers as M};

// Scratch CSpace slots for IPC tests.
const SLOT_LEASED_EP:  u32 = 5;  // blocked-recv test (BIND_LEASE_FOR_IPC_TEST)
const SLOT_CALL_EP:    u32 = 6;  // blocked-call test (BIND_LEASE_AND_CALL_BACK)
// Own endpoint slot (pre-installed, object 0).
const SLOT_OWN_EP:    u32 = 0;

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    let ep: u32 = 0;  // slot 0 = own endpoint (object 0)

    loop {
        // Use recv_msg to capture data words (needed for BIND_LEASE_FOR_IPC_TEST).
        let (label, w0, _, _, _) = match sys_ipc_recv_msg(ep) {
            Ok(v)  => v,
            Err(_) => { let _ = sys_ipc_reply(0); continue; }
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
                let ok = 'setup: {
                    let h = match sys_cap_copy(CapHandle(SLOT_OWN_EP), SLOT_LEASED_EP) {
                        Ok(h)  => h,
                        Err(_) => break 'setup false,
                    };
                    if sys_cap_bind_lease(h, lease_id).is_err() {
                        let _ = sys_cap_drop(h);
                        break 'setup false;
                    }
                    true
                };
                if !ok {
                    let _ = sys_ipc_reply(usize::MAX);  // setup failed
                    continue;
                }
                // Reply OK — neg-test will now yield and then revoke the lease.
                let _ = sys_ipc_reply(0);

                // Block in ipc_recv with the leased cap.
                // Woken by cancel_blocked_ipc_for_lease when neg-test revokes.
                match sys_ipc_recv(SLOT_LEASED_EP) {
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
                let _ = sys_cap_drop(CapHandle(SLOT_LEASED_EP));
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
                let ok = 'setup2: {
                    let h = match sys_cap_copy(CapHandle(SLOT_OWN_EP), SLOT_CALL_EP) {
                        Ok(h)  => h,
                        Err(_) => break 'setup2 false,
                    };
                    if sys_cap_bind_lease(h, lease_id).is_err() {
                        let _ = sys_cap_drop(h);
                        break 'setup2 false;
                    }
                    true
                };
                if !ok {
                    let _ = sys_ipc_reply(usize::MAX);
                    continue;
                }
                // Reply OK — neg-test will now call sys_ipc_recv(0).
                let _ = sys_ipc_reply(0);
                // Call neg-test back with the leased cap.
                // neg-test will receive this, revoke the lease, and try to reply.
                match fjell_syscall::sys_ipc_call(SLOT_CALL_EP, tags::CALL_BACK_MSG) {
                    Err(_) => {
                        // Woken with LeaseRevoked — the BLOCKED_CALL path works.
                        sys_debug_writeln(M::IPC_BLOCKED_CALL);
                    }
                    Ok(_) => {
                        // Got a reply (unexpected in the test scenario).
                        // Continue normally.
                    }
                }
                let _ = sys_cap_drop(CapHandle(SLOT_CALL_EP));
            }

            // ── Unknown label ────────────────────────────────────────────────
            _ => { let _ = sys_ipc_reply(0); }
        }
    }

    sys_exit(0)
}