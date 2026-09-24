//! `svc-presentation-fault` — a test-only presentation that dies mid-run
//! (RFC-0.34-001 D11).
//!
//! The case E-058 measured at 229 lines was a presentation faulting **after** it
//! had been working. This is that case as a committed tier, the way `svc-fault`
//! is a service that faults on purpose: it registers with the stream the way any
//! presentation does — by asking — is woken when there is something to show,
//! takes its first message, and faults. `init` starts it only when the console
//! byte `P` is injected, so no other profile has it, and its only power is to
//! be a presentation that stops.
//!
//! It exists so that the crash case is **evidence in the tree** rather than a
//! scratch build with a fault edited in by hand.

#![no_std]
#![no_main]
mod rt;

use fjell_service_api::presentation;
use fjell_syscall::{sys_debug_writeln, sys_ipc_recv_msg};

/// This task's own endpoint: where the stream's wake arrives.
const EP_SLOT: u32 = 0;
/// A CALL capability to `semantic-stream` (object 7).
const STREAM_EP: u32 = 1;

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    loop {
        match presentation::ask(STREAM_EP) {
            presentation::Ask::Message { .. } => {
                // An intention, printed before the fault, not an outcome.
                sys_debug_writeln("svc-presentation-fault: faulting on its first message");
                // SAFETY: category=raw-pointer-deref deliberate null read: this
                // task exists to fault, so that a presentation that dies
                // mid-run is observable in a tier.
                // MMIO-ORDER: poll
                // (Not an MMIO access. The tag is how the MMIO audit is told so
                // for a deliberate volatile null read, exactly as `svc-fault`'s.)
                let _ = unsafe { core::ptr::read_volatile(core::ptr::null::<u8>()) };
            }
            presentation::Ask::Empty => {
                // Parked, as every presentation does; the wake brings the next ask.
                let _ = sys_ipc_recv_msg(EP_SLOT);
            }
        }
    }
}
