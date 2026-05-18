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
        // M3 capability syscalls
        Some(SyscallNumber::CapCopy) |
        Some(SyscallNumber::CapMint) |
        Some(SyscallNumber::CapDelete) |
        Some(SyscallNumber::CapRevoke) |
        Some(SyscallNumber::CapInspect) |
        // M3 IPC syscalls
        Some(SyscallNumber::IpcSend) |
        Some(SyscallNumber::IpcRecv) |
        Some(SyscallNumber::IpcCall) |
        Some(SyscallNumber::IpcReply)   => dispatch_m3(tf, nr),
        // M4 task syscalls
        Some(SyscallNumber::TaskSpawn)  => dispatch_task_spawn(tf),
        Some(SyscallNumber::TaskStart)  => dispatch_task_start(tf),
        Some(SyscallNumber::TaskStatus) => dispatch_task_status(tf),
        // M4 lease syscalls
        Some(SyscallNumber::LeaseCreate)  => dispatch_lease_create(tf),
        Some(SyscallNumber::LeaseRevoke)  => dispatch_lease_revoke(tf),
        Some(SyscallNumber::LeaseInspect) => dispatch_lease_inspect(tf),
        // M4 audit
        Some(SyscallNumber::AuditDrain)   => sys_audit_drain(tf),
        // DebugWrite handled before
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

/// `sys_debug_write(a0=byte)` — write one byte to UART (smoke-test helper).
///
/// Used by `fjell_syscall::sys_debug_write_byte` in user-space services.
fn sys_debug_write(tf: &mut TrapFrame) {
    let b = tf.gpr[REG_A0] as u8;
    // Direct MMIO write; safe because UART PA is identity-mapped.
    unsafe { (0x1000_0000usize as *mut u8).write_volatile(b) };
    tf.gpr[REG_A0] = SysError::Ok as usize;
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

// ── M3 syscall dispatcher ─────────────────────────────────────────────────────

fn dispatch_m3(tf: &mut TrapFrame, nr: usize) {
    use crate::cap::syscall::*;
    use fjell_abi::syscall::SyscallNumber;

    // Get kernel state — SAFETY: single-hart, initialised before first trap.
    let (table, sched, ct, et) = unsafe { crate::get_kernel_state() };

    // Determine the calling task's index from sscratch → TrapFrame ptr.
    // For M3 we derive task index from the scheduler's current().
    let cur_id = sched.current().unwrap_or(crate::task::TaskId::new(0, 0));
    let tidx   = cur_id.index as usize;

    match SyscallNumber::from_usize(nr) {
        Some(SyscallNumber::CapCopy)    => sys_cap_copy(tf, tidx, ct),
        Some(SyscallNumber::CapMint)    => sys_cap_mint(tf, tidx, ct),
        Some(SyscallNumber::CapDelete)  => sys_cap_delete(tf, tidx, ct),
        Some(SyscallNumber::CapRevoke)  => sys_cap_revoke(tf, tidx, ct),
        Some(SyscallNumber::CapInspect) => sys_cap_inspect(tf, tidx, ct),
        Some(SyscallNumber::IpcSend)    => sys_ipc_send(tf, tidx, ct, et, table, sched, cur_id),
        Some(SyscallNumber::IpcRecv)    => sys_ipc_recv(tf, tidx, ct, et, table, sched, cur_id),
        Some(SyscallNumber::IpcCall)    => sys_ipc_call(tf, tidx, ct, et, table, sched, cur_id),
        Some(SyscallNumber::IpcReply)   => sys_ipc_reply(tf, tidx, ct, table, sched),
        _                               => { /* already handled in caller */ }
    }
}

// ── M4 syscall handlers ───────────────────────────────────────────────────────

/// `sys_task_spawn(a0=image_id) -> a0=task_handle_raw, a1=0`
pub fn sys_task_spawn(
    tf: &mut TrapFrame,
    table:       &mut crate::task::tcb::TaskTable,
    sched:       &mut crate::task::scheduler::Scheduler,
    kernel_root: crate::mm::frame_alloc::PhysFrame,
    fa:          *mut crate::mm::frame_alloc::FrameAllocator<'static>,
) {
    use fjell_abi::service::ImageId;
    use crate::task::spawn::spawn;
    let image_id = ImageId(tf.gpr[REG_A0] as u16);
    let fa_ref = unsafe { &mut *fa };
    match spawn(image_id, table, sched, kernel_root, fa_ref) {
        Ok(tid) => {
            tf.gpr[REG_A0] = 0;                          // SysError::Ok
            tf.gpr[REG_A1] = tid.index as usize;         // task handle
        }
        Err(e) => {
            tf.gpr[REG_A0] = e as isize as usize;
        }
    }
}

/// `sys_task_start(a0=handle, a1=entry_pc, a2=stack_top)`
pub fn sys_task_start(
    tf:    &mut TrapFrame,
    table: &mut crate::task::tcb::TaskTable,
    sched: &mut crate::task::scheduler::Scheduler,
) {
    use fjell_abi::error::SysError;
    use crate::task::TaskId; use crate::task::tcb::TaskState;
    let handle = tf.gpr[REG_A0] as u16;
    let entry  = tf.gpr[10 + 1]; // a1
    let stack  = tf.gpr[10 + 2]; // a2
    let tid    = TaskId::new(handle, 0);
    match table.get_mut(tid) {
        Some(task) => {
            if task.state != TaskState::Created {
                tf.gpr[REG_A0] = SysError::PermissionDenied as isize as usize;
                return;
            }
            if entry != 0 { task.trap_frame.sepc   = entry; }
            if stack != 0 { task.trap_frame.gpr[2] = stack; }
            task.state = TaskState::Runnable;
            sched.enqueue_runnable(tid, 2 /* PRIORITY_USER */);
            tf.gpr[REG_A0] = 0;
        }
        None => { tf.gpr[REG_A0] = SysError::InvalidCap as isize as usize; }
    }
}

/// `sys_task_status(a0=handle) -> a0=lifecycle_byte`
pub fn sys_task_status(tf: &mut TrapFrame, table: &crate::task::tcb::TaskTable) {
    use fjell_abi::error::SysError;
    use fjell_abi::service::TaskLifecycle;
    use crate::task::TaskId; use crate::task::tcb::TaskState;
    let tid = TaskId::new(tf.gpr[REG_A0] as u16, 0);
    match table.get(tid) {
        Some(task) => {
            let lc = match task.state {
                TaskState::Empty    => { tf.gpr[REG_A0] = SysError::InvalidCap as isize as usize; return; }
                TaskState::Created    => TaskLifecycle::Created,
                TaskState::Runnable   => TaskLifecycle::Runnable,
                TaskState::Running    => TaskLifecycle::Running,
                TaskState::Blocked(_) => TaskLifecycle::Blocked,
                TaskState::Exited(_)  => TaskLifecycle::Exited,
                TaskState::Faulted(_) => TaskLifecycle::Faulted,
            };
            tf.gpr[REG_A0] = lc as usize;
        }
        None => { tf.gpr[REG_A0] = SysError::InvalidCap as isize as usize; }
    }
}

/// `sys_lease_create(a0=flags) -> a0=LeaseId.0`
pub fn sys_lease_create(tf: &mut TrapFrame, lt: &mut crate::lease::LeaseTable,
                         tidx: usize) {
    use fjell_abi::task::TaskId;
    let flags = tf.gpr[REG_A0] as u32;
    let owner = TaskId::new(tidx as u16, 0);
    match lt.create(owner, flags) {
        Ok(id) => { tf.gpr[REG_A0] = 0; tf.gpr[REG_A1] = id.0 as usize; }
        Err(e) => { tf.gpr[REG_A0] = e as isize as usize; }
    }
}

/// `sys_lease_revoke(a0=lease_id) -> a0=new_epoch`
pub fn sys_lease_revoke(tf: &mut TrapFrame, lt: &mut crate::lease::LeaseTable) {
    use fjell_abi::lease::LeaseId;
    let id = LeaseId(tf.gpr[REG_A0] as u32);
    match lt.revoke(id) {
        Ok(ep) => { tf.gpr[REG_A0] = 0; tf.gpr[REG_A1] = ep.0 as usize; }
        Err(e) => { tf.gpr[REG_A0] = e as isize as usize; }
    }
}

/// `sys_lease_inspect(a0=lease_id) -> a0=epoch`
pub fn sys_lease_inspect(tf: &mut TrapFrame, lt: &crate::lease::LeaseTable) {
    use fjell_abi::lease::LeaseId;
    let id = LeaseId(tf.gpr[REG_A0] as u32);
    match lt.current_epoch(id) {
        Ok(ep) => { tf.gpr[REG_A0] = ep.0 as usize; }
        Err(e) => { tf.gpr[REG_A0] = e as isize as usize; }
    }
}

/// `sys_audit_drain(a0=buf_ptr, a1=buf_len) -> a0=bytes_written`
pub fn sys_audit_drain(tf: &mut TrapFrame) {
    tf.gpr[REG_A0] = 0;
}

// ── M4 dispatch wrappers ──────────────────────────────────────────────────────

fn dispatch_task_spawn(tf: &mut TrapFrame) {
    use crate::mm::frame_alloc::PhysFrame;
    let (table, sched, _ct, _et) = unsafe { crate::get_kernel_state() };
    let pfn = crate::KERNEL_ROOT_PFN.load(core::sync::atomic::Ordering::Relaxed);
    let kernel_root = PhysFrame::from_pfn(pfn as u64).unwrap();
    // SAFETY: single-hart, kernel frame allocator accessed exclusively.
    let fa_ptr = unsafe { crate::fa_static_ptr() };
    sys_task_spawn(tf, table, sched, kernel_root, fa_ptr);
}

fn dispatch_task_start(tf: &mut TrapFrame) {
    let (table, sched, _ct, _et) = unsafe { crate::get_kernel_state() };
    sys_task_start(tf, table, sched);
}

fn dispatch_task_status(tf: &mut TrapFrame) {
    let (table, _, _ct, _et) = unsafe { crate::get_kernel_state() };
    sys_task_status(tf, table);
}

fn dispatch_lease_create(tf: &mut TrapFrame) {
    let lt   = unsafe { crate::get_lease_table() };
    let tidx = crate::trap::dispatch::current_task_idx();
    sys_lease_create(tf, lt, tidx);
}

fn dispatch_lease_revoke(tf: &mut TrapFrame) {
    let lt = unsafe { crate::get_lease_table() };
    sys_lease_revoke(tf, lt);
}

fn dispatch_lease_inspect(tf: &mut TrapFrame) {
    let lt = unsafe { crate::get_lease_table() };
    sys_lease_inspect(tf, lt);
}
