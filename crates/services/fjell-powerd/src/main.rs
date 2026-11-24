//! Power / sustainability telemetry skeleton — M6.
#![no_std]
#![no_main]
mod rt;
use fjell_syscall::{sys_debug_writeln, sys_exit};

// SAFETY: category=symbol-export `service_main` is this service's entry symbol, tail-called by `_start` in rt.rs; no other item in the image exports the name.
#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    // M6: skeleton — observes but does not yet optimise.
    sys_debug_writeln("M6: powerd started");
    sys_exit(0)
}
