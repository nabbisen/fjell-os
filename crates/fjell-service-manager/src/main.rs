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
    name: &'static str,
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
    let ep = 0u32;

    // Announce ready to init
    let _ = sys_ipc_reply(tags::SERVICE_READY);

    // Process start-services request from init
    match sys_ipc_recv(ep) {
        Ok(tags::SM_START_SERVICE) => {}
        _ => sys_exit(1),
    }

    let mut graph = make_graph();

    // Spawn and start each service
    for svc in graph.iter_mut() {
        match sys_task_spawn(svc.image) {
            Ok((handle, _ctrl_cap)) => {
                svc.handle = handle;
                let _ = sys_task_start(handle, 0, 0);
                svc.state = SvcState::Spawned;
            }
            Err(_) => {
                svc.state = SvcState::Failed;
            }
        }
    }

    // Wait briefly for services to reach Running state
    for _ in 0..100 {
        let mut all_ready = true;
        for svc in graph.iter_mut() {
            if svc.state == SvcState::Spawned {
                if let Ok(s) = sys_task_status(svc.handle) {
                    if s == TaskLifecycle::Running as u8 || s == TaskLifecycle::Runnable as u8 {
                        svc.state = SvcState::Running;
                    }
                }
            }
            if svc.state != SvcState::Ready && svc.state != SvcState::Running {
                all_ready = false;
            }
        }
        if all_ready { break; }
        // Small cooperative yield
        for _ in 0..1000 { unsafe { core::arch::asm!("nop") }; }
    }

    // Promote Running → Ready
    for svc in graph.iter_mut() {
        if svc.state == SvcState::Running { svc.state = SvcState::Ready; }
    }

    // Report core.target ready
    let _ = sys_ipc_reply(tags::SM_CORE_TARGET_READY);

    // Serve status queries
    loop {
        match sys_ipc_recv(ep) {
            Ok(tags::SM_STATUS_QUERY) => {
                // Pack count of ready services into reply
                let ready = graph.iter().filter(|s| s.state == SvcState::Ready).count();
                let _ = sys_ipc_reply(tags::SM_STATUS_REPLY | (ready << 12));
            }
            Ok(tags::SERVICE_SHUTDOWN) => break,
            Ok(_) | Err(_) => { let _ = sys_ipc_reply(0); }
        }
    }

    sys_exit(0)
}
