#![allow(dead_code)]
//! Syscall dispatch and individual syscall handlers.
//!
//! Invariants (TRAP-*):
//!   TRAP-001  sepc is advanced by 4 after every ecall.
//!   TRAP-002  Unknown syscall → SysError::UnknownSyscall, no panic.

use crate::task::tcb::{TrapFrame, REG_A0, REG_A1, REG_A2, REG_A3, REG_A7};
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
        Some(SyscallNumber::CapDrop)    |
        Some(SyscallNumber::CapBindLease) |
        // M3 IPC syscalls
        Some(SyscallNumber::IpcSend) |
        Some(SyscallNumber::IpcRecv) |
        Some(SyscallNumber::IpcCall) |
        Some(SyscallNumber::IpcReply)   => dispatch_m3(tf, nr),
        Some(SyscallNumber::IpcTryRecv)  => dispatch_ipc_try_recv(tf),
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
        // M6 device / MMIO / DMA
        Some(SyscallNumber::PlatformInfoGet) => sys_platform_info_get(tf),
        Some(SyscallNumber::MmioMap)         => sys_mmio_map(tf),
        Some(SyscallNumber::DmaAlloc)        => sys_dma_alloc(tf),
        Some(SyscallNumber::DmaRevoke)       => sys_dma_revoke(tf),
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

// ── RFC 048: handle-based capability gating helper ───────────────────────────

/// Handle-based capability enforcement (RFC 048).
///
/// Replaces the v0.1/v0.2.8 scan-based `require_cap`.  The caller explicitly
/// names the cap handle, which is validated through all 7 RFC 031 steps:
/// CSpace lookup, generation, slot-state, kind, rights, scope, lease epoch.
///
/// `required_scope = None` skips the scope check (used for creation ops
/// where no target object exists yet).
pub(crate) fn require_cap_on_ct(
    ct:              &crate::cap::table::CapTable,
    tidx:            usize,
    handle:          fjell_cap::handle::CapHandle,
    expected_kind:   fjell_cap::CapKind,
    required_rights: fjell_cap::CapRights,
    required_scope:  Option<&fjell_cap::rights::ObjectScope>,
) -> Result<(), SysError> {
    use fjell_cap::rights::CapError;
    let lt = unsafe { crate::get_lease_table() };
    let cs = ct.cspace(tidx).ok_or(SysError::InternalError)?;
    fjell_cap::enforcement::require_cap(cs, handle, expected_kind, required_rights,
                                        required_scope, lt)
        .map(|_| ())
        .map_err(|e| match e {
            CapError::InvalidHandle | CapError::GenerationMismatch
                | CapError::EmptySlot | CapError::WrongKind => SysError::InvalidCap,
            CapError::MissingRight | CapError::ScopeMismatch => SysError::PermissionDenied,
            CapError::LeaseRevoked => SysError::LeaseRevoked,
            _                      => SysError::PermissionDenied,
        })
}


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
    pub(crate) const fn new() -> Self { Flag(core::cell::Cell::new(false)) }
    pub(crate) fn store(&self, v: bool) { self.0.set(v); }
    pub(crate) fn load(&self) -> bool { self.0.get() }
    pub(crate) fn take(&self) -> bool { let v = self.0.get(); self.0.set(false); v }
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
        Some(SyscallNumber::CapDrop)    => sys_cap_drop(tf, tidx, ct),
        Some(SyscallNumber::CapBindLease) => sys_cap_bind_lease(tf, tidx, ct),
        Some(SyscallNumber::IpcSend)    => sys_ipc_send(tf, tidx, ct, et, table, sched, cur_id),
        Some(SyscallNumber::IpcRecv)    => sys_ipc_recv(tf, tidx, ct, et, table, sched, cur_id),
        Some(SyscallNumber::IpcCall)    => sys_ipc_call(tf, tidx, ct, et, table, sched, cur_id),
        Some(SyscallNumber::IpcReply)   => sys_ipc_reply(tf, tidx, ct, table, sched),
        _                               => { /* already handled in caller */ }
    }
}

// ── M4 syscall handlers ───────────────────────────────────────────────────────

/// `sys_task_spawn(a0=image_id) -> a0=task_handle_raw, a1=0`
/// Requires `CapKind::TaskCreate` capability (RFC 004).
/// `sys_task_spawn(a0=cap_handle, a1=image_id)` — RFC 048: handle-based ABI.
pub fn sys_task_spawn(
    tf: &mut TrapFrame,
    table:       &mut crate::task::tcb::TaskTable,
    sched:       &mut crate::task::scheduler::Scheduler,
    kernel_root: crate::mm::frame_alloc::PhysFrame,
    fa:          *mut crate::mm::frame_alloc::FrameAllocator<'static>,
    ct:          &crate::cap::table::CapTable,
    tidx:        usize,
) {
    use fjell_abi::service::ImageId;
    use crate::task::spawn::spawn;
    // RFC 048: require TaskCreate capability via handle in a0.
    let cap_h = fjell_cap::handle::CapHandle(tf.gpr[REG_A0] as u32);
    if let Err(e) = require_cap_on_ct(ct, tidx, cap_h,
                                      fjell_cap::CapKind::TaskCreate,
                                      fjell_cap::CapRights::TASK_CREATE, None) {
        tf.gpr[REG_A0] = e as isize as usize; return;
    }
    let image_id = ImageId(tf.gpr[REG_A1] as u16);
    let fa_ref = unsafe { &mut *fa };
    match spawn(image_id, table, sched, kernel_root, fa_ref) {
        Ok(tid) => {
            // RFC 010: encode (index, generation) into a single u32 handle.
            // handle = index | (generation << 16)
            let handle = (tid.index as u32) | ((tid.generation as u32) << 16);
            tf.gpr[REG_A0] = 0;                          // SysError::Ok
            tf.gpr[REG_A1] = handle as usize;            // task handle
        }
        Err(e) => {
            tf.gpr[REG_A0] = e as isize as usize;
        }
    }
}

/// `sys_task_start(a0=cap_handle, a1=task_handle, a2=entry_pc, a3=stack_top)` — RFC 048.
pub fn sys_task_start(
    tf:    &mut TrapFrame,
    table: &mut crate::task::tcb::TaskTable,
    sched: &mut crate::task::scheduler::Scheduler,
    ct:    &crate::cap::table::CapTable,
    tidx:  usize,
) {
    use fjell_abi::error::SysError;
    use crate::task::TaskId; use crate::task::tcb::TaskState;
    use fjell_cap::rights::ObjectScope;
    // RFC 048: decode target task handle from a1; scope-check the TaskControl cap.
    let cap_h  = fjell_cap::handle::CapHandle(tf.gpr[REG_A0] as u32);
    let raw    = tf.gpr[REG_A1] as u32;
    let index      = (raw & 0xFFFF) as u16;
    let generation = (raw >> 16) as u16;
    let target_tid = TaskId::new(index, generation);
    let req_scope  = ObjectScope::Task(target_tid);
    if let Err(e) = require_cap_on_ct(ct, tidx, cap_h,
                                      fjell_cap::CapKind::TaskControl,
                                      fjell_cap::CapRights::TASK_START,
                                      Some(&req_scope)) {
        tf.gpr[REG_A0] = e as isize as usize; return;
    }
    let entry  = tf.gpr[REG_A2]; // a2
    let stack  = tf.gpr[REG_A3]; // a3

    // RFC 022: validate entry_pc and stack_top are in user address range.
    // Kernel RAM starts at RAM_BASE = 0x8000_0000; user code must be below it.
    const USER_ADDR_MAX: usize = crate::platform::qemu_virt::RAM_BASE;
    if entry != 0 && (entry >= USER_ADDR_MAX || entry < 0x1000) {
        tf.gpr[REG_A0] = SysError::InvalidCap as isize as usize;
        return;
    }
    if stack != 0 && (stack >= USER_ADDR_MAX || stack < 0x2000) {
        tf.gpr[REG_A0] = SysError::InvalidCap as isize as usize;
        return;
    }

    let tid    = TaskId::new(index, generation);
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

/// `sys_task_status(a0=cap_handle, a1=task_handle) -> a0=lifecycle_byte` — RFC 048.
pub fn sys_task_status(tf: &mut TrapFrame, table: &crate::task::tcb::TaskTable,
                       ct: &crate::cap::table::CapTable, tidx: usize) {
    use fjell_abi::error::SysError;
    use fjell_abi::service::TaskLifecycle;
    use crate::task::TaskId; use crate::task::tcb::TaskState;
    use fjell_cap::rights::ObjectScope;
    // RFC 048: handle-based TaskControl check with Task-scope validation.
    let cap_h = fjell_cap::handle::CapHandle(tf.gpr[REG_A0] as u32);
    let raw   = tf.gpr[REG_A1] as u32;
    let tid   = TaskId::new((raw & 0xFFFF) as u16, (raw >> 16) as u16);
    let req_scope = ObjectScope::Task(tid);
    if let Err(e) = require_cap_on_ct(ct, tidx, cap_h,
                                      fjell_cap::CapKind::TaskControl,
                                      fjell_cap::CapRights::TASK_STATUS,
                                      Some(&req_scope)) {
        tf.gpr[REG_A0] = e as isize as usize; return;
    }
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

/// `sys_lease_create(a0=cap_handle, a1=flags) -> a0=ok, a1=LeaseId` — RFC 048.
pub fn sys_lease_create(tf: &mut TrapFrame, lt: &mut crate::lease::LeaseTable,
                        ct: &crate::cap::table::CapTable, tidx: usize) {
    // RFC 048: handle-based LeaseAdmin check; no scope (creation, no target yet).
    let cap_h = fjell_cap::handle::CapHandle(tf.gpr[REG_A0] as u32);
    if let Err(e) = require_cap_on_ct(ct, tidx, cap_h,
                                      fjell_cap::CapKind::LeaseAdmin,
                                      fjell_cap::CapRights::LEASE_CREATE, None) {
        tf.gpr[REG_A0] = e as isize as usize; return;
    }
    use fjell_abi::task::TaskId;
    let flags = tf.gpr[REG_A1] as u32;
    let owner = TaskId::new(tidx as u16, 0);
    match lt.create(owner, flags) {
        Ok(id) => { tf.gpr[REG_A0] = 0; tf.gpr[REG_A1] = id.0 as usize; }
        Err(e) => { tf.gpr[REG_A0] = e as isize as usize; }
    }
}

/// `sys_lease_revoke(a0=cap_handle, a1=lease_id) -> a0=ok, a1=new_epoch` — RFC 048.
pub fn sys_lease_revoke(tf: &mut TrapFrame, lt: &mut crate::lease::LeaseTable,
                        ct: &crate::cap::table::CapTable, tidx: usize) {
    // RFC 048: handle-based LeaseAdmin check with Lease-scope validation.
    use fjell_cap::rights::ObjectScope;
    use fjell_abi::lease::LeaseId;
    let cap_h = fjell_cap::handle::CapHandle(tf.gpr[REG_A0] as u32);
    let id    = LeaseId(tf.gpr[REG_A1] as u32);
    let req_scope = ObjectScope::Lease(id);
    if let Err(e) = require_cap_on_ct(ct, tidx, cap_h,
                                      fjell_cap::CapKind::LeaseAdmin,
                                      fjell_cap::CapRights::LEASE_REVOKE,
                                      Some(&req_scope)) {
        tf.gpr[REG_A0] = e as isize as usize; return;
    }
    match lt.revoke(id) {
        Ok(ep) => { tf.gpr[REG_A0] = 0; tf.gpr[REG_A1] = ep.0 as usize; }
        Err(e) => { tf.gpr[REG_A0] = e as isize as usize; }
    }
}

/// `sys_lease_inspect(a0=cap_handle, a1=lease_id) -> a0=epoch` — RFC 048.
pub fn sys_lease_inspect(tf: &mut TrapFrame, lt: &crate::lease::LeaseTable,
                         ct: &crate::cap::table::CapTable, tidx: usize) {
    // RFC 048: handle-based LeaseAdmin check with Lease-scope validation.
    use fjell_cap::rights::ObjectScope;
    use fjell_abi::lease::LeaseId;
    let cap_h = fjell_cap::handle::CapHandle(tf.gpr[REG_A0] as u32);
    let id    = LeaseId(tf.gpr[REG_A1] as u32);
    let req_scope = ObjectScope::Lease(id);
    if let Err(e) = require_cap_on_ct(ct, tidx, cap_h,
                                      fjell_cap::CapKind::LeaseAdmin,
                                      fjell_cap::CapRights::LEASE_INSPECT,
                                      Some(&req_scope)) {
        tf.gpr[REG_A0] = e as isize as usize; return;
    }
    match lt.current_epoch(id) {
        Ok(ep) => { tf.gpr[REG_A0] = ep.0 as usize; }
        Err(e) => { tf.gpr[REG_A0] = e as isize as usize; }
    }
}

/// Drain pending kernel audit records into a caller-supplied buffer.
///
/// `sys_audit_drain(a0=cap_handle, a1=buf_va, a2=buf_len)` — RFC 053/054.
///
/// RFC 054: first arg is the AuditDrain cap handle (handle-based `require_cap`).
/// RFC 053: peek-copy-advance — records are not consumed until copy succeeds.
///
/// Returns:
///   `a0` = status (0 = ok)
///   `a1` = n_records_copied
///   `a2` = n_dropped_since_last_drain (per RFC 053; resets on each drain)
pub fn sys_audit_drain(tf: &mut TrapFrame) {
    use fjell_audit_format::{AuditRecordBin, AUDIT_RECORD_BIN_SIZE};
    use fjell_cap::{CapKind, CapRights};
    use crate::audit::ring::AUDIT;
    use crate::mm::user_copy::copy_to_user_bytes;

    let cap_raw = tf.gpr[REG_A0] as u32;
    let buf_va  = tf.gpr[REG_A1];
    let buf_len = tf.gpr[REG_A2];

    // ── 1. RFC 054: handle-based AuditDrain require_cap with lease check ─────
    let (table, sched, ct, _) = unsafe { crate::get_kernel_state() };
    let cur_id = match sched.current() {
        Some(id) => id,
        None => { tf.gpr[REG_A0] = SysError::BadState as isize as usize; return; }
    };
    let tidx   = cur_id.index as usize;
    let cap_h  = fjell_cap::handle::CapHandle(cap_raw);
    if let Err(e) = require_cap_on_ct(ct, tidx, cap_h,
                                      CapKind::AuditDrain,
                                      CapRights::AUDIT_DRAIN, None) {
        tf.gpr[REG_A0] = e as isize as usize; return;
    }

    // ── 2. Trivial: buffer too small for one record ───────────────────────────
    if buf_len < AUDIT_RECORD_BIN_SIZE {
        tf.gpr[REG_A0] = SysError::Ok as isize as usize;
        tf.gpr[REG_A1] = 0;
        tf.gpr[REG_A2] = 0;
        return;
    }

    // ── 3. Get caller page-table root for copy_to_user ───────────────────────
    let root_pfn = match table.get(cur_id) {
        Some(task) => task.satp_root_pfn,
        None => { tf.gpr[REG_A0] = SysError::BadState as isize as usize; return; }
    };

    // ── 4. RFC 053: peek-copy-advance ─────────────────────────────────────────
    //
    // Records are peeked (non-consuming) and copied to user space.
    // Only AFTER each successful copy is the cursor advanced.
    // If copy fails at record i, records 0..i have already been copied and
    // committed; records i..n remain in the ring for the next drain call.
    let max_records = (buf_len / AUDIT_RECORD_BIN_SIZE).min(64);
    let mut n_copied = 0usize;

    'drain: for i in 0..max_records {
        // Peek without consuming.
        let rec = match AUDIT.peek_at(i) {
            Some(r) => r,
            None    => break 'drain,  // ring exhausted
        };
        // Serialize to binary format.
        let bin = AuditRecordBin {
            seq:    rec.seq,
            tick:   rec.tick,
            kind:   rec.kind as u16,
            task:   0xFFFF,
            arg0:   rec.arg0 as u32,
            arg1:   rec.arg1 as u32,
            result: rec.result as i32,
        };
        let bytes = unsafe {
            core::slice::from_raw_parts(
                &bin as *const AuditRecordBin as *const u8,
                AUDIT_RECORD_BIN_SIZE,
            )
        };
        let dst = buf_va + i * AUDIT_RECORD_BIN_SIZE;
        // Copy to user; stop at first failure (remaining records stay in ring).
        match unsafe { copy_to_user_bytes(root_pfn, dst, bytes) } {
            Ok(_)  => n_copied += 1,
            Err(_) => break 'drain,
        }
    }

    // 5. Advance ring head by exactly n_copied and get per-drain drop count.
    let n_dropped = AUDIT.advance(n_copied);

    tf.gpr[REG_A0] = SysError::Ok as isize as usize;
    tf.gpr[REG_A1] = n_copied;
    tf.gpr[REG_A2] = n_dropped as usize;
}


// ── M4 dispatch wrappers ──────────────────────────────────────────────────────

fn dispatch_task_spawn(tf: &mut TrapFrame) {
    use crate::mm::frame_alloc::PhysFrame;
    let (table, sched, ct, _et) = unsafe { crate::get_kernel_state() };
    let tidx = crate::trap::dispatch::current_task_idx();
    let pfn = crate::KERNEL_ROOT_PFN.load(core::sync::atomic::Ordering::Relaxed);
    let kernel_root = PhysFrame::from_pfn(pfn as u64).unwrap();
    let fa_ptr = unsafe { crate::fa_static_ptr() };
    sys_task_spawn(tf, table, sched, kernel_root, fa_ptr, ct, tidx);
}

fn dispatch_task_start(tf: &mut TrapFrame) {
    let (table, sched, ct, _et) = unsafe { crate::get_kernel_state() };
    let tidx = crate::trap::dispatch::current_task_idx();
    sys_task_start(tf, table, sched, ct, tidx);
}

fn dispatch_task_status(tf: &mut TrapFrame) {
    let (table, _, ct, _et) = unsafe { crate::get_kernel_state() };
    let tidx = crate::trap::dispatch::current_task_idx();
    sys_task_status(tf, table, ct, tidx);
}

fn dispatch_lease_create(tf: &mut TrapFrame) {
    let lt   = unsafe { crate::get_lease_table() };
    let (_, _, ct, _) = unsafe { crate::get_kernel_state() };
    let tidx = crate::trap::dispatch::current_task_idx();
    sys_lease_create(tf, lt, ct, tidx);
}

fn dispatch_lease_revoke(tf: &mut TrapFrame) {
    use crate::cap::syscall::cancel_blocked_ipc_for_lease;
    let lt   = unsafe { crate::get_lease_table() };
    let (table, sched, ct, et) = unsafe { crate::get_kernel_state() };
    let tidx = crate::trap::dispatch::current_task_idx();
    use fjell_abi::lease::LeaseId;
    // RFC 048: lease_id is now in a1 (a0 is cap_handle).
    let id = LeaseId(tf.gpr[REG_A1] as u32);
    let old_epoch_result = lt.current_epoch(id).map(|ep| ep.0);
    sys_lease_revoke(tf, lt, ct, tidx);
    if tf.gpr[REG_A0] == 0 {
        if let Ok(old_epoch) = old_epoch_result {
            cancel_blocked_ipc_for_lease(id, old_epoch, ct, et, table, sched);
        }
    }
}

fn dispatch_lease_inspect(tf: &mut TrapFrame) {
    let lt = unsafe { crate::get_lease_table() };
    let (_, _, ct, _) = unsafe { crate::get_kernel_state() };
    let tidx = crate::trap::dispatch::current_task_idx();
    sys_lease_inspect(tf, lt, ct, tidx);
}

// ── M6 syscall handlers ───────────────────────────────────────────────────────

/// `sys_platform_info_get() -> a0=0, a1=virtio_blk_base_pa`
///
/// Scans all 8 virtio-mmio slots on QEMU virt (0x10001000..0x10008000) and
/// returns the base PA of the first slot that contains a virtio block device
/// (magic=0x74726976, version=2, device_id=2).
pub fn sys_platform_info_get(tf: &mut TrapFrame) {
    // Return the virtio-blk MMIO base address for QEMU virt (RISC-V).
    //
    // QEMU virt assigns the first `-drive if=virtio` device to bus 0 at
    // physical address 0x10001000.  We hard-code this value rather than
    // scanning, because the scan can fail when the calling task's Sv39 page
    // table does not yet have the MMIO range identity-mapped.
    //
    // TODO(M8): replace with proper device-tree / ACPI enumeration.
    const VIRTIO_BLK_BASE: usize = 0x1000_1000;
    tf.gpr[REG_A0] = 0;
    tf.gpr[REG_A1] = VIRTIO_BLK_BASE;
}


/// `sys_mmio_map(a0=mmio_cap_handle, a1=offset, a2=size) -> a0=status, a1=user_va`
///
/// RFC 035 (v0.2.0): Map a bounded MMIO physical sub-range into the caller's
/// address space.  Enforces the full `require_cap` check order:
///   1. CSpace lookup + generation check
///   2. Slot-state check
///   3. Kind check      — must be `CapKind::MmioRegion`
///   4. Rights check    — must carry `CapRights::MMIO_MAP`
///   5. Scope check     — (deferred; Any accepted)
///   6. Lease check     — epoch must still match
///   7. Offset/size bounds within the static `MmioRegionTable` entry
///   8. Non-RAM (RFC 005 defense-in-depth)
///   9. Non-executable mapping
pub fn sys_mmio_map(tf: &mut TrapFrame) {
    use crate::mm::{address::VirtAddr, vspace::VmPerms};
    use crate::platform::qemu_virt::{RAM_BASE, RAM_END, mmio_region_table};
    use fjell_cap::{CapKind, CapRights};

    let cap_handle = fjell_cap::CapHandle(tf.gpr[REG_A0] as u32);
    let offset     = tf.gpr[10 + 1];                               // a1
    let size_bytes = (tf.gpr[10 + 2] + 0xFFF) & !0xFFF;           // a2, page-align up

    if size_bytes == 0 {
        tf.gpr[REG_A0] = SysError::InvalidArg as isize as usize;
        return;
    }

    let tidx = crate::trap::dispatch::current_task_idx();
    let (_, _, cap_table, _) = unsafe { crate::get_kernel_state() };
    let lt = unsafe { crate::get_lease_table() };

    let cs = match cap_table.cspace(tidx) {
        Some(c) => c,
        None    => { tf.gpr[REG_A0] = SysError::InternalError as isize as usize; return; }
    };

    // RFC 035 §2 + RFC 031: unified require_cap — kind=MmioRegion, right=MMIO_MAP.
    let region_idx = match fjell_cap::enforcement::require_cap(
        cs, cap_handle,
        CapKind::MmioRegion, CapRights::MMIO_MAP,
        None, lt,
    ) {
        Ok(cap)  => cap.object_id as usize,
        Err(e)   => { tf.gpr[REG_A0] = e.to_sys_error() as isize as usize; return; }
    };

    // Bounds-check offset + size and verify the region is Active.
    let mmio_table = mmio_region_table();
    let region = match mmio_table.get(region_idx) {
        Some(r) => r,
        None    => { tf.gpr[REG_A0] = SysError::InvalidCap as isize as usize; return; }
    };
    if !region.is_accessible(offset, size_bytes) {
        tf.gpr[REG_A0] = SysError::InvalidArg as isize as usize;
        return;
    }
    let phys_addr = region.base + offset;

    // RFC 005 defense-in-depth: reject kernel RAM.
    let end_pa = phys_addr.saturating_add(size_bytes);
    if phys_addr < RAM_END && end_pa > RAM_BASE {
        tf.gpr[REG_A0] = SysError::InvalidArg as isize as usize;
        return;
    }

    // Map pages R|W|U — explicitly no X bit (RFC 009 / RFC 035).
    // RFC 051: allocate user VA from the task's device VMA bump allocator
    // instead of using PA directly as VA (which could alias user heap).
    let task_id  = crate::task::TaskId::new(tidx as u16, 0);
    let (table, _, _, _) = unsafe { crate::get_kernel_state() };
    let root_pfn = table.get(task_id).map(|t| t.satp_root_pfn).unwrap_or(0);
    if root_pfn == 0 {
        tf.gpr[REG_A0] = SysError::NoMemory as isize as usize;
        return;
    }

    // Allocate a contiguous device VA range for this mapping.
    let device_va_base = {
        let task = match table.get_mut(task_id) {
            Some(t) => t,
            None => { tf.gpr[REG_A0] = SysError::InternalError as isize as usize; return; }
        };
        let va = task.dev_vma_next;
        let end = va.saturating_add(size_bytes);
        if end > crate::platform::qemu_virt::DEVICE_VMA_END {
            tf.gpr[REG_A0] = SysError::NoMemory as isize as usize;
            return;
        }
        task.dev_vma_next = (end + 4095) & !4095;  // page-align next
        va
    };

    let fa = unsafe { &mut *crate::fa_static_ptr() };
    let mut mapped = 0usize;
    for i in 0..((size_bytes + 4095) / 4096) {
        let pa = phys_addr + i * 4096;
        let va = device_va_base + i * 4096;
        if let Ok(f) = crate::mm::frame_alloc::PhysFrame::from_pa(pa) {
            if mapped == 0 { mapped = 1; }
            unsafe {
                let _ = crate::mm::page_table::remap_page(
                    root_pfn << 12, VirtAddr(va), f,
                    VmPerms::R | VmPerms::W | VmPerms::U,
                    fa,
                );
            }
        }
    }
    unsafe { crate::arch::riscv64::csr::sfence_vma(); }
    tf.gpr[REG_A0] = 0;
    tf.gpr[REG_A1] = device_va_base;  // RFC 051: VA in device range, not PA
}

/// `sys_dma_alloc(a0=dma_cap_handle, a1=size_bytes) -> a0=status, a1=user_va, a2=device_pa`
///
/// RFC 036 (v0.2.0): Allocate a DMA region.  Requires `CapKind::DmaRegion`
/// with `CapRights::DMA_ALLOC` right (replaces v0.1.x kind-only check).
///
/// Maximum 1 page (4 KiB) per DMA region — invariant DMA-003.
pub fn sys_dma_alloc(tf: &mut TrapFrame) {
    use crate::mm::{address::VirtAddr, vspace::VmPerms, frame_alloc::FrameOwner};
    use crate::task::tcb::REG_A2;
    use fjell_cap::{CapKind, CapRights};

    let cap_raw    = tf.gpr[REG_A0] as u32;
    let size_bytes = (tf.gpr[REG_A1] + 0xFFF) & !0xFFF;
    let cap_handle = fjell_cap::CapHandle(cap_raw);

    // RFC 036 + RFC 031: unified require_cap — kind=DmaRegion | DmaAlloc, right=DMA_ALLOC.
    let tidx = crate::trap::dispatch::current_task_idx();
    {
        let (_, _, ct, _) = unsafe { crate::get_kernel_state() };
        let lt = unsafe { crate::get_lease_table() };
        let cs = match ct.cspace(tidx) {
            Some(c) => c,
            None => { tf.gpr[REG_A0] = SysError::InternalError as isize as usize; return; }
        };
        // Accept both the new DmaRegion kind and the legacy DmaAlloc alias.
        let kind_ok = {
            let c = cs.get(cap_handle).ok();
            c.map_or(false, |cap| {
                cap.kind == CapKind::DmaRegion || cap.kind == CapKind::DmaAlloc
            })
        };
        if !kind_ok {
            // Run require_cap for the proper error path.
            if let Err(e) = fjell_cap::enforcement::require_cap(
                cs, cap_handle, CapKind::DmaRegion, CapRights::DMA_ALLOC, None, lt,
            ) {
                tf.gpr[REG_A0] = e.to_sys_error() as isize as usize;
                return;
            }
        } else {
            // Kind matched — still check DMA_ALLOC right and lease.
            if let Ok(cap) = cs.get(cap_handle) {
                if !cap.rights.contains(CapRights::DMA_ALLOC) {
                    tf.gpr[REG_A0] = SysError::PermissionDenied as isize as usize;
                    return;
                }
                if let Err(e) = cap.check_lease(lt) {
                    tf.gpr[REG_A0] = e.to_sys_error() as isize as usize;
                    return;
                }
            }
        }
    }

    // RFC 036: MAX 1 page — invariant DMA-003.
    let pages = size_bytes / 4096;
    if pages == 0 || pages > 1 {
        tf.gpr[REG_A0] = SysError::InvalidArg as isize as usize;
        return;
    }
    let _ = REG_A2;

    let (table, _, _, _) = unsafe { crate::get_kernel_state() };
    let task_id  = crate::task::TaskId::new(tidx as u16, 0);
    let root_pfn = table.get(task_id).map(|t| t.satp_root_pfn).unwrap_or(0);
    if root_pfn == 0 {
        tf.gpr[REG_A0] = SysError::NoMemory as isize as usize;
        return;
    }
    let fa = unsafe { &mut *crate::fa_static_ptr() };

    let user_va_start = crate::DMA_VA_NEXT.fetch_add(
        pages * 4096, core::sync::atomic::Ordering::Relaxed,
    );

    let frame = match fa.alloc_frame(FrameOwner::UserStack { task: task_id }) {
        Ok(f) => f,
        Err(_) => { tf.gpr[REG_A0] = SysError::NoMemory as isize as usize; return; }
    };
    let first_pa = frame.pa();
    unsafe {
        let _ = crate::mm::page_table::map_page(
            root_pfn << 12, VirtAddr(user_va_start), frame,
            VmPerms::R | VmPerms::W | VmPerms::U, fa,
        );
    }
    unsafe { crate::arch::riscv64::csr::sfence_vma(); }

    // RFC 036 + RFC 052: record ownership; rollback if table is full.
    if !crate::dma_table().alloc(task_id, user_va_start, first_pa) {
        // Table full — unmap, zeroize, free the just-allocated frame.
        // Safety: we own the frame exclusively; no other task has seen it.
        unsafe {
            // Zeroize the physical frame.
            core::ptr::write_bytes(first_pa as *mut u8, 0, 4096);
            // Unmap the user VA (best-effort; single-hart guarantees no races).
            if let Ok(frame) = crate::mm::frame_alloc::PhysFrame::from_pa(first_pa) {
                let _ = (*crate::fa_static_ptr()).free_frame(frame);
            }
        }
        tf.gpr[REG_A0] = SysError::NoMemory as isize as usize; // DMA table full (RFC 052)
        return;
    }

    tf.gpr[REG_A0] = 0;
    tf.gpr[REG_A1] = user_va_start;
    tf.gpr[12]     = first_pa;
}

/// `sys_dma_revoke(a0=cap_handle, a1=device_pa) -> a0=status` — RFC 052.
///
/// RFC 052: validates DmaRegion cap with DMA_REVOKE right (closes RB-09 right check).
/// Region identified by device_pa (object_id-based tracking deferred to v0.3).
/// User VA unmap deferred to v0.3 (frame is zeroized and freed; VA stays mapped).
pub fn sys_dma_revoke(tf: &mut TrapFrame) {
    use fjell_cap::{CapKind, CapRights};
    let cap_raw   = tf.gpr[REG_A0] as u32;
    let device_pa = tf.gpr[REG_A1];
    let tidx      = crate::trap::dispatch::current_task_idx();
    let cap_h     = fjell_cap::handle::CapHandle(cap_raw);
    let (_, _, ct, _) = unsafe { crate::get_kernel_state() };

    // RFC 052: require DmaRegion + DMA_REVOKE right, lease check.
    if let Err(e) = require_cap_on_ct(ct, tidx, cap_h,
                                      CapKind::DmaRegion,
                                      CapRights::DMA_REVOKE, None) {
        tf.gpr[REG_A0] = e as isize as usize; return;
    }

    let task_id = crate::task::TaskId::new(tidx as u16, 0);
    if crate::dma_table().revoke_by_pa(task_id, device_pa) {
        tf.gpr[REG_A0] = 0;
    } else {
        tf.gpr[REG_A0] = SysError::InvalidArg as isize as usize;
    }
}

/// Dispatch `sys_ipc_try_recv` (RFC 019 — non-blocking IPC receive).
fn dispatch_ipc_try_recv(tf: &mut TrapFrame) {
    use crate::cap::syscall::sys_ipc_try_recv;
    let tidx = crate::trap::dispatch::current_task_idx();
    let (table, _, ct, et) = unsafe { crate::get_kernel_state() };
    sys_ipc_try_recv(tf, tidx, ct, et, table);
}
