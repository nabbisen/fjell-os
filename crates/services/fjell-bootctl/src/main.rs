//! Boot-control service — RFC 057.
//!
//! Slot layout:
//!   0 = Endpoint (shared, object 0)
//!   1 = Reboot cap (CapKind::Reboot, REBOOT right)
#![no_std]
#![no_main]
mod rt;

use fjell_syscall::{sys_exit, sys_ipc_recv, sys_ipc_reply, sys_debug_writeln, sys_reboot};
use fjell_service_api::tags;
use fjell_cap::CapHandle;

const SLOT_OWN_EP: u32 = 0;
const SLOT_REBOOT: u32 = 1;

#[derive(Clone, Copy, PartialEq)]
enum BootState { Pending, Confirmed, Rollback }

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("bootctl: started (RFC 057)");
    let mut state = BootState::Pending;
    loop {
        let label = match sys_ipc_recv(SLOT_OWN_EP) {
            Ok(l)  => l & 0xFFFF,
            Err(_) => { let _ = sys_ipc_reply(usize::MAX); continue; }
        };
        match label {
            l if l == (tags::BOOT_PENDING_QUERY & 0xFFFF) => {
                let s = match state { BootState::Pending=>0, BootState::Confirmed=>1, BootState::Rollback=>2 };
                let _ = sys_ipc_reply(tags::BOOT_STATE_REPLY | (s << 16));
            }
            l if l == (tags::BOOT_CONFIRM & 0xFFFF) => {
                if state == BootState::Pending { state = BootState::Confirmed; sys_debug_writeln("bootctl: CONFIRMED"); let _ = sys_ipc_reply(0); }
                else { let _ = sys_ipc_reply(usize::MAX); }
            }
            l if l == (tags::BOOT_ROLLBACK & 0xFFFF) => {
                #[allow(unused_assignments)]  // state set for semantic correctness; reboot follows immediately
                { state = BootState::Rollback; }
                sys_debug_writeln("bootctl: ROLLBACK");
                let _ = sys_ipc_reply(0);
                let _ = sys_reboot(CapHandle(SLOT_REBOOT), 0);
                loop {}
            }
            l if l == (tags::BOOT_SHUTDOWN & 0xFFFF) => { let _ = sys_ipc_reply(0); sys_exit(0); }
            _ => { let _ = sys_ipc_reply(usize::MAX); }
        }
    }
}
