//! Service manager stub.
//!
//! Starts core services in dependency order, monitors readiness, and
//! tracks service lifecycle states.
//!
//! This is an M7.1 stub — real IPC-based service orchestration is M8 work.

#![no_std]
#![no_main]
mod rt;

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    // M7.1 stub: service-manager is a cooperative idle loop.
    // Real IPC-based service orchestration is M8 work.
    loop {
        let _ = fjell_syscall::sys_ipc_recv(0u32);
    }
}
