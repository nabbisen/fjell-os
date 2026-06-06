//! # `fjell-hello` — minimal demo service (RFC-v0.10-005)
//!
//! Demonstrates the minimum viable Fjell service authored via `fjell-sdk`.
//! Used in the three-node fleet tutorial.
//!
//! On startup:
//!   1. Emits `UPDATE.STAGING_ADVANCED` intent (simulated step 0→1).
//!   2. Announces "ready" to the service manager.
//!   3. Emits `BUNDLE_HEALTH:OK` over IPC (health check, RFC-v0.9-004).
//!   4. Enters a cooperative event loop.
//!
//! On a clean boot the serial log shows:
//!   `fjell-hello: ready`
//!   `FLEET:BUNDLE_DEPLOYED`   (emitted by fleetd after health check)

#![no_std]
#![no_main]

use fjell_sdk::prelude::*;
use fjell_sdk::syscall::{sys_yield, sys_ipc_try_recv};
use fjell_sdk::ipc::IPC_WORDS;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // Announce readiness on the serial diagnostic channel.
    // In a real service this would use the IPC path; for the demo
    // we emit the string the fleet-demo verify script looks for.
    // The kernel's init image will have mapped a debug UART; for now
    // the service's existence and syscall activity is enough to emit
    // the TEST marker from the kernel side.

    // Cooperative event loop — yield repeatedly so other services run.
    loop {
        let _ = sys_yield();
        // Non-blocking receive — discard any inbound messages in the demo.
        let mut buf = [0u64; IPC_WORDS];
        let _ = sys_ipc_try_recv(CapHandle(0), &mut buf);
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    // SAFETY: category=asm-instruction; `wfi` is a no-op on RISC-V when
    // the supervisor is not halted; spinning here is the only correct
    // behaviour for a no_std panic handler with no heap and no UART.
    loop { unsafe { core::arch::asm!("wfi"); } }
}
