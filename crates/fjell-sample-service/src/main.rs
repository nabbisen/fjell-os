//! Sample user-space service for M4 smoke test.
//!
//! Sends a READY signal to the service-manager endpoint (CSpace slot 0),
//! then enters an idle loop responding to heartbeat requests.

#![no_std]
#![no_main]
mod rt;

use fjell_syscall::{sys_exit, sys_ipc_recv, sys_ipc_reply};
use fjell_service_api::tags;

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    // Slot 0 = service control endpoint (pre-installed by kernel/service-manager)
    let ep = 0u32;

    // Announce readiness
    // M7.1: startup IPC reply removed (nobody IpcCall'd us).

    // Serve requests
    loop {
        match sys_ipc_recv(ep) {
            Ok(tags::SERVICE_HEARTBEAT) => { let _ = sys_ipc_reply(tags::SERVICE_HEARTBEAT); }
            Ok(tags::SERVICE_SHUTDOWN)  => break,
            Ok(_) | Err(_)              => { let _ = sys_ipc_reply(0); }
        }
    }

    sys_exit(0)
}
