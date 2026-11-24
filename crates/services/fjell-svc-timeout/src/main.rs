//! RFC 042: svc-timeout test service.
//!
//! Intentionally never sends a READY signal.  Used by neg-test to verify that
//! the start-timeout path is detectable: after N yields, neg-test checks this
//! task's status — it will still be Runnable/Blocked (alive), indicating that
//! READY was never received within the expected window.
//!
//! **RFC-0.48-001 (the boot-lease demonstration).** It also probes its one boot capability -- the `SEND`-only endpoint
//! to `service-manager` every service is granted at `SERVICE_READY_SEND_SLOT` -- through the kernel's funnel, **without
//! delivering anything**: a send whose tag claims 255 words is refused `MsgTooLong` *after* the funnel has accepted the
//! capability, and refused `LeaseRevoked` *by* the funnel once the capability's lease is revoked. So a live probe never
//! signals READY (this service must never), and the first `LeaseRevoked` is the moment `neg-test` -- which spawned this
//! task and so owns its boot lease -- took the authority back. It prints one line when it sees that, and nothing else.
#![no_std]
#![no_main]
mod rt;

use fjell_abi::error::SysError;
use fjell_abi::service::SERVICE_READY_SEND_SLOT;
use fjell_syscall::{sys_debug_writeln, sys_ipc_send, sys_yield};

/// A tag whose word count (bits 16..23) is 255: the kernel refuses it as `MsgTooLong` once the capability is accepted.
const INVALID_TAG: usize = 0xFF << 16;

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    let mut reported = false;
    loop {
        if !reported {
            match sys_ipc_send(SERVICE_READY_SEND_SLOT, INVALID_TAG) {
                Err(SysError::LeaseRevoked) => {
                    sys_debug_writeln("NEG:BOOTLEASE:CHILD_SEND_REFUSED_LEASE_REVOKED:PASS");
                    reported = true;
                }
                // The funnel accepted the capability and the (deliberately invalid) message was refused: alive.
                Err(SysError::MsgTooLong) => {}
                // Anything else means the probe is not testing what it says it tests.
                _ => {
                    sys_debug_writeln("NEG:HARNESS:WRONG_ERROR");
                    reported = true;
                }
            }
        }
        sys_yield();
    }
}
