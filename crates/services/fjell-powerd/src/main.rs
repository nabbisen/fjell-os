//! Power / sustainability telemetry skeleton — M6.
#![no_std]
#![no_main]
mod rt;
use fjell_syscall::{sys_exit, sys_debug_writeln};

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    // M6: skeleton — observes but does not yet optimise.
    sys_debug_writeln("M6: powerd started");
    sys_exit(0)
}
