//! RFC 042: svc-fault test service.
//!
//! Yields once (simulating startup work), then deliberately causes a RISC-V
//! page fault by reading from virtual address 0.  The kernel marks this task
//! as `TaskState::Faulted`; neg-test detects the fault via `sys_task_status`
//! and emits `NEG:SVC:FAULT_DETECTED:PASS`.
#![no_std]
#![no_main]
mod rt;

use fjell_syscall::{sys_debug_write, sys_debug_writeln, sys_yield};

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    // Yield once so neg-test can spawn-and-yield in the right order.
    sys_yield();

    // RFC-0.34-002 D6: this is a TEST service, and the console is what it exercises
    // here. (1) A line longer than the kernel's 160-byte console buffer: it must be
    // shown split, the chunks saying so (`[cont]` ends the first, begins the second),
    // not as two lines with nothing between them. 22 + 138 = 160 bytes, then a tail.
    sys_debug_write("svc-fault: long line: ");
    for _ in 0..138 {
        sys_debug_write("a");
    }
    sys_debug_writeln("TAIL-END");
    // (2) A task that leaves mid-line: these words end with no newline and the task
    // faults below. They must be SHOWN, ending `[cut]` — before the fault this line
    // was lost silently, because nothing flushed a task's buffer when it left.
    sys_debug_write("svc-fault: last words before the fault");

    // Deliberately fault: read from null pointer → page fault → kernel marks Faulted.
    // SAFETY: category=raw-pointer-deref intentional fault for negative testing.
    // MMIO-ORDER: poll
    let _ = unsafe { core::ptr::read_volatile(0usize as *const u8) };

    // Unreachable — fault above will trap and the kernel will Faulted the task.
    loop {
        sys_yield();
    }
}
