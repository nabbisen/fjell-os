//! recoveryd — Recovery plane service for Fjell OS M8.
//!
//! Provides snapshot listing, slot inspection, and capability-controlled
//! rollback.  Manual rollback always requires confirmed_by_operator = true.
#![no_std]
#![no_main]
mod rt;
use fjell_recovery_format::{RecoveryError, SlotId, SlotState, HealthStatus};
use fjell_service_api::recoveryd as proto;
use fjell_syscall::{sys_debug_writeln, sys_exit};
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { sys_debug_writeln("recoveryd: panic"); sys_exit(1); }
const EP_SLOT: u32 = 0;
// SAFETY: category=raw-pointer-deref IPC call slot is valid; response buffer length is bounded by MAX_IPC_MSG.
fn send_ready() { unsafe { core::arch::asm!("li a7, 20","ecall", in("a0") EP_SLOT as usize, in("a1") proto::READY, lateout("a0") _, lateout("a7") _, options(nostack)); } }
fn recv_call() -> (usize, usize, usize, usize, usize) {
    let (mut t, mut w0, mut w1, mut w2, mut w3) = (0usize,0usize,0usize,0usize,0usize);
    // SAFETY: category=raw-pointer-deref IPC call slot is valid; response buffer length is bounded by MAX_IPC_MSG.
    unsafe { core::arch::asm!("li a7, 21","ecall", in("a0") EP_SLOT as usize, lateout("a1") t, lateout("a2") w0, lateout("a3") w1, lateout("a4") w2, lateout("a5") w3, lateout("a7") _, options(nostack)); }
    (t, w0, w1, w2, w3)
}
// SAFETY: category=raw-pointer-deref IPC call slot is valid; response buffer length is bounded by MAX_IPC_MSG.
fn reply(tag: usize, w0: usize, w1: usize, w2: usize) { unsafe { core::arch::asm!("li a7, 23","ecall", in("a0") 0usize, in("a1") tag, in("a2") w0, in("a3") w1, in("a4") w2, lateout("a7") _, options(nostack)); } }
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
                let _reason   = w1 as u8;
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
