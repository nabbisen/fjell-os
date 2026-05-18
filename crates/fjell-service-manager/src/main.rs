//! Service manager for M4.
//!
//! Starts core services in dependency order, monitors readiness, and
//! tracks service lifecycle states.

#![no_std]
#![no_main]
mod rt;

use fjell_syscall::{sys_exit, sys_ipc_recv, sys_ipc_reply,
                    sys_task_spawn, sys_task_start, sys_task_status};
use fjell_abi::service::{ImageId, TaskLifecycle};
use fjell_service_api::tags;

// ── Service graph (M4 bootstrap, hardcoded) ───────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum SvcState { Pending, Spawned, Running, Ready, Failed }

struct ServiceEntry {
    #[allow(dead_code)] name: &'static str,
    image: ImageId,
    state: SvcState,
    handle: usize,
}

fn make_graph() -> [ServiceEntry; 1] {
    [
        ServiceEntry {
            name:  "svc.sample",
            image: ImageId::SAMPLE_SERVICE,
            state: SvcState::Pending,
            handle: 0,
        },
    ]
}

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    // M7.1 stub: service-manager is a cooperative idle loop.
    // Real IPC-based service orchestration is M8 work.
    loop {
        let _ = fjell_syscall::sys_ipc_recv(0u32);
    }
}
