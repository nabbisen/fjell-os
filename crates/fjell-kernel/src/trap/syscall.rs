#![allow(dead_code)]
//! Syscall dispatch and individual syscall handlers.
//!
//! Invariants (TRAP-*):
//!   TRAP-001  sepc is advanced by 4 after every ecall.
//!   TRAP-002  Unknown syscall → SysError::UnknownSyscall, no panic.

use crate::task::tcb::{TrapFrame, REG_A0, REG_A1, REG_A7};
use fjell_abi::{error::SysError, syscall::SyscallNumber};

/// Dispatch a syscall.
///
/// Reads `a7` from the trap frame, routes to the appropriate handler, then
/// advances `sepc` by 4 (TRAP-001).
pub fn handle_syscall(tf: &mut TrapFrame) {
    let nr = tf.gpr[REG_A7];

    match SyscallNumber::from_usize(nr) {
        Some(SyscallNumber::Yield)      => sys_yield(tf),
        Some(SyscallNumber::Exit)       => sys_exit(tf),
        Some(SyscallNumber::DebugWrite) => sys_debug_write(tf),
        Some(_) | None => {
            // TRAP-002: unknown syscall is not a kernel panic.
            tf.gpr[REG_A0] = SysError::UnknownSyscall as isize as usize;
            // Record in audit ring if available.
            crate::audit::ring::AUDIT.lock_free_append(
                crate::audit::ring::AuditKindInternal::UnknownSyscall,
                nr,
                0,
                SysError::UnknownSyscall as isize,
            );
        }
    }

    // Advance past the ecall instruction (TRAP-001).
    tf.sepc = tf.sepc.wrapping_add(4);
}

// ── Individual syscall implementations ───────────────────────────────────────

/// `sys_yield` — voluntarily relinquish the CPU.
///
/// The actual task switch happens in the kernel main loop after
/// `trap_dispatch` returns; this handler just marks the intent.
fn sys_yield(tf: &mut TrapFrame) {
    // Signal to the main loop that a yield was requested by setting a
    // well-known sentinel in a caller-saved register (a0 = Ok).
    tf.gpr[REG_A0] = SysError::Ok as usize;
    // The kernel run-loop detects yield via CURRENT_TASK_YIELDED flag.
    YIELD_REQUESTED.store(true);
}

/// `sys_exit` — terminate the calling task.
fn sys_exit(tf: &mut TrapFrame) {
    let code = tf.gpr[10] as i32; // a0 = exit code
    tf.gpr[REG_A0] = SysError::Ok as usize;
    EXIT_CODE.store(code);
    EXIT_REQUESTED.store(true);
}

/// `sys_debug_write` — write bytes to the UART (smoke-test only).
fn sys_debug_write(tf: &mut TrapFrame) {
    // a0 = pointer to bytes (user VA), a1 = length.
    // In M2 we trust the pointer (no capability check); this syscall is
    // removed or protected in M3+.
    let _ptr = tf.gpr[REG_A0];
    let _len = tf.gpr[REG_A1];
    tf.gpr[REG_A0] = SysError::Ok as usize;
    // Actual output omitted: we cannot safely dereference user pointers
    // without page-table walk in M2.  Smoke test relies on kernel-side prints.
}

// ── Shared state between syscall handler and kernel run-loop ─────────────────

/// Simple non-atomic flag (single-hart M2; no concurrency).
pub(crate) struct Flag(core::cell::Cell<bool>);
// SAFETY: single-hart, no concurrent access in M2.
unsafe impl Sync for Flag {}
impl Flag {
    const fn new() -> Self { Flag(core::cell::Cell::new(false)) }
    fn store(&self, v: bool) { self.0.set(v); }
    fn load(&self) -> bool { self.0.get() }
    fn take(&self) -> bool { let v = self.0.get(); self.0.set(false); v }
}

pub(crate) struct I32Cell(core::cell::Cell<i32>);
// SAFETY: single-hart.
unsafe impl Sync for I32Cell {}
impl I32Cell {
    const fn new() -> Self { I32Cell(core::cell::Cell::new(0)) }
    fn store(&self, v: i32) { self.0.set(v); }
    fn load(&self) -> i32 { self.0.get() }
}

pub(crate) static YIELD_REQUESTED: Flag = Flag::new();
pub(crate) static EXIT_REQUESTED:  Flag = Flag::new();
pub(crate) static EXIT_CODE:        I32Cell = I32Cell::new();

/// Called by the timer handler to request a preemptive yield.
pub fn request_yield() { YIELD_REQUESTED.store(true); }
/// Called by trap_dispatch to check if the current task yielded.
pub fn take_yield() -> bool { YIELD_REQUESTED.take() }
/// Called by the kernel run-loop to check if the current task exited.
pub fn take_exit() -> Option<i32> {
    if EXIT_REQUESTED.take() { Some(EXIT_CODE.load()) } else { None }
}
