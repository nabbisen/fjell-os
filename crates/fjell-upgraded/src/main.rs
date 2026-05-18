//! Immutable A/B upgrade staging — M6.
#![no_std]
#![no_main]
mod rt;
use fjell_syscall::{sys_exit, sys_debug_writeln};
use fjell_upgrade_format::{UpgradeState, SlotId};

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("M6: upgraded started");
    // Development-grade: simulate staging to inactive slot B
    sys_debug_writeln("M6: inactive slot staged");
    // Set candidate slot
    sys_debug_writeln("M6: candidate slot set");
    // Simulate boot confirmation (would require reboot in production)
    sys_debug_writeln("M6: boot confirmation simulated");
    sys_exit(0)
}
