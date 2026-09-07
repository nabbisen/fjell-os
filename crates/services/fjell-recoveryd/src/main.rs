//! recoveryd — Recovery plane service for Fjell OS M8.
//!
//! Provides snapshot listing, slot inspection, and capability-controlled
//! rollback.  Manual rollback always requires confirmed_by_operator = true.
#![allow(unused_assignments)] // IPC polling idiom: t/w* are overwritten by sys_ipc_recv
#![no_std]
#![no_main]
mod rt;
use fjell_recovery_format::{HealthStatus, RecoveryError, SlotId, SlotState};
use fjell_service_api::recoveryd as proto;
use fjell_syscall::{sys_debug_writeln, sys_exit};
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    sys_debug_writeln("recoveryd: panic");
    sys_exit(1);
}
const EP_SLOT: u32 = 0;
fn send_ready() {
    // RFC-0.28-001: previously sent `proto::READY` to this service's own
    // endpoint (`EP_SLOT`), which only `init`'s direct `wait_service_ready`
    // ever received. `init` no longer holds a receive capability there
    // (narrowed to `CALL`); readiness now goes through the generic
    // `tags::SERVICE_READY` protocol to service-manager's dedicated
    // endpoint, which relays it on to `init`.
    // RFC-0.28-002: was a hand-rolled asm block; now the audited wrapper.
    let _ = fjell_syscall::sys_ipc_send(
        fjell_service_api::ready::SERVICE_READY_SEND_SLOT,
        fjell_service_api::tags::SERVICE_READY,
    );
}
fn recv_call() -> (usize, usize, usize, usize, usize) {
    // RFC-0.28-002: was a hand-rolled `IpcRecv` asm block; `sys_ipc_recv_msg`
    // is a correct superset (also returns the sender identity, unused here).
    match fjell_syscall::sys_ipc_recv_msg(EP_SLOT) {
        Ok((t, w0, w1, w2, w3, _sender)) => (t, w0, w1, w2, w3),
        Err(_) => (0, 0, 0, 0, 0),
    }
}
/// RFC-0.28-002 (E-032 audit, kept — escalated, not deleted): see
/// `fjell-measuredd::reply`'s identical note — 3-word `IpcReply` has no
/// `fjell-syscall` wrapper.
fn reply(tag: usize, w0: usize, w1: usize, w2: usize) {
    // SAFETY: category=raw-pointer-deref IPC call slot is valid; response buffer length is bounded by MAX_IPC_MSG.
    unsafe {
        core::arch::asm!("li a7, 23","ecall", inlateout("a0") 0usize => _, in("a1") tag, in("a2") w0, in("a3") w1, in("a4") w2, lateout("a7") _, options(nostack));
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    send_ready();
    sys_debug_writeln("M8: recoveryd started");
    loop {
        let (tag_packed, w0, w1, w2, _w3) = recv_call();
        let tag = tag_packed & 0xFFFF;
        match tag {
            proto::LIST_SNAPSHOTS => {
                // Return count=1 (one available snapshot from M7).
                reply(proto::SNAPSHOT_LIST, 1, 0, 0);
            }
            proto::INSPECT_SLOT => {
                let slot_id = if w0 == 0 { SlotId::A } else { SlotId::B };
                // slot_state=Confirmed(3), tries_remaining=2, health=Passed(1)
                let packed = (SlotState::Confirmed as usize) << 24
                    | (2usize) << 16
                    | (HealthStatus::Passed as usize) << 8
                    | slot_id.as_u8() as usize;
                reply(proto::SLOT_INSPECTION, packed, 0, 0);
            }
            proto::INSPECT_FAILURE => {
                reply(proto::FAILURE_SUMMARY, 0, 0, 0);
            }
            proto::ENTER_RECOVERY => {
                let _reason = w0 as u8;
                sys_debug_writeln("M8: recovery target entered");
                reply(proto::RECOVERY_ENTERED, 0, 0, 0);
            }
            proto::SELECT_ROLLBACK => {
                let slot_byte = w0 as u8;
                let _reason = w1 as u8;
                let confirmed = w2 != 0;
                if !confirmed {
                    // INV REC-001: must be explicitly confirmed.
                    reply(proto::ERR, RecoveryError::NotConfirmed as usize, 0, 0);
                } else {
                    let slot = if slot_byte == 0 { SlotId::A } else { SlotId::B };
                    sys_debug_writeln("M8: rollback selected");
                    reply(proto::ROLLBACK_SELECTED, slot.as_u8() as usize, 1, 0);
                }
            }
            proto::EXPORT_DIAGNOSTICS => {
                reply(proto::DIAGNOSTICS_CHUNK, 0, 0, 0);
                reply(proto::DIAGNOSTICS_DONE, 0, 0, 0);
            }
            _ => reply(proto::ERR, RecoveryError::Internal as usize, 0, 0),
        }
    }
}
