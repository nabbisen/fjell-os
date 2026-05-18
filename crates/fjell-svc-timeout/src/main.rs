//! RFC 042: svc-timeout test service.
//!
//! Intentionally never sends a READY signal.  Used by neg-test to verify that
//! the start-timeout path is detectable: after N yields, neg-test checks this
//! task's status — it will still be Runnable/Blocked (alive), indicating that
//! READY was never received within the expected window.
#![no_std]
#![no_main]
mod rt;

use fjell_syscall::sys_yield;

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    // Spin forever — intentionally never sends READY.
    loop { sys_yield(); }
}
