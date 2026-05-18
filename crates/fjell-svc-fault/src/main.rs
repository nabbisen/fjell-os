//! RFC 042: svc-fault test service.
//!
//! Yields once (simulating startup work), then deliberately causes a RISC-V
//! page fault by reading from virtual address 0.  The kernel marks this task
//! as `TaskState::Faulted`; neg-test detects the fault via `sys_task_status`
//! and emits `NEG:SVC:FAULT_DETECTED:PASS`.
#![no_std]
#![no_main]
mod rt;

use fjell_syscall::sys_yield;

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    // Yield once so neg-test can spawn-and-yield in the right order.
    sys_yield();

    // Deliberately fault: read from null pointer → page fault → kernel marks Faulted.
    // SAFETY: intentional fault for negative testing.
    let _ = unsafe { core::ptr::read_volatile(0usize as *const u8) };

    // Unreachable — fault above will trap and the kernel will Faulted the task.
    loop { sys_yield(); }
}
