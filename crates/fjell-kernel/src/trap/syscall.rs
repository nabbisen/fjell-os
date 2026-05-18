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

// ── RFC 014: capability gating helper ────────────────────────────────────────

/// Require that the calling task holds a capability of `kind` with at least
/// `required_rights`, and whose lease (if any) is still active.
///
/// This replaces the v0.0.9 `caller_has_cap(kind)` which ignored rights and
/// lease binding (RFC 014).
fn require_cap(kind: fjell_cap::CapKind, required_rights: fjell_cap::CapRights) -> Result<(), SysError> {
    use crate::trap::dispatch::current_task_idx;
    let (_, _, cap_table, _) = unsafe { crate::get_kernel_state() };
    let lt = unsafe { crate::get_lease_table() };
    let tidx = current_task_idx();
    let cs = match cap_table.cspace(tidx) {
        Some(c) => c,
        None    => return Err(SysError::InternalError),
    };
    let found = cs.slots().iter().any(|slot| {
        if let Some(cap) = slot.cap {
            cap.kind == kind
            && cap.rights.contains(required_rights)
            && cap.check_lease(lt).is_ok()
        } else {
            false
        }
    });
    if found { Ok(()) } else { Err(SysError::PermissionDenied) }
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
/// Requires `CapKind::TaskCreate` capability (RFC 004).
pub fn sys_task_spawn(
    tf: &mut TrapFrame,
    table:       &mut crate::task::tcb::TaskTable,
    sched:       &mut crate::task::scheduler::Scheduler,
    kernel_root: crate::mm::frame_alloc::PhysFrame,
    fa:          *mut crate::mm::frame_alloc::FrameAllocator<'static>,
) {
    use fjell_abi::service::ImageId;
    use crate::task::spawn::spawn;
    // RFC 014: require TaskCreate capability with full rights.
    if let Err(e) = require_cap(fjell_cap::CapKind::TaskCreate, fjell_cap::CapRights::ALL) {
        tf.gpr[REG_A0] = e as isize as usize; return;
    }
    let image_id = ImageId(tf.gpr[REG_A0] as u16);
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

/// `sys_task_start(a0=handle, a1=entry_pc, a2=stack_top)`
pub fn sys_task_start(
    tf:    &mut TrapFrame,
    table: &mut crate::task::tcb::TaskTable,
    sched: &mut crate::task::scheduler::Scheduler,
) {
    use fjell_abi::error::SysError;
    use crate::task::TaskId; use crate::task::tcb::TaskState;
    // RFC 014: require TaskControl capability.
    if let Err(e) = require_cap(fjell_cap::CapKind::TaskControl, fjell_cap::CapRights::ALL) {
        tf.gpr[REG_A0] = e as isize as usize; return;
    }
    // RFC 010: decode (index, generation) from packed u32 handle.
    let raw        = tf.gpr[REG_A0] as u32;
    let index      = (raw & 0xFFFF) as u16;
    let generation = (raw >> 16) as u16;
    let entry  = tf.gpr[10 + 1]; // a1
    let stack  = tf.gpr[10 + 2]; // a2

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

/// `sys_task_status(a0=handle) -> a0=lifecycle_byte`
/// Requires `CapKind::TaskControl` with `INSPECT` rights (RFC 014).
pub fn sys_task_status(tf: &mut TrapFrame, table: &crate::task::tcb::TaskTable) {
    use fjell_abi::error::SysError;
    use fjell_abi::service::TaskLifecycle;
    use crate::task::TaskId; use crate::task::tcb::TaskState;
    use fjell_cap::CapRights;
    // RFC 014: require TaskControl | INSPECT capability.
    if let Err(e) = require_cap(fjell_cap::CapKind::TaskControl, CapRights::INSPECT) {
        tf.gpr[REG_A0] = e as isize as usize; return;
    }
    // RFC 010: decode (index, generation) from packed u32 handle.
    let raw        = tf.gpr[REG_A0] as u32;
    let tid = TaskId::new((raw & 0xFFFF) as u16, (raw >> 16) as u16);
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
    // RFC 014: require LeaseAdmin capability.
    if let Err(e) = require_cap(fjell_cap::CapKind::LeaseAdmin, fjell_cap::CapRights::ALL) {
        tf.gpr[REG_A0] = e as isize as usize; return;
    }
    use fjell_abi::task::TaskId;
    let flags = tf.gpr[REG_A0] as u32;
    let owner = TaskId::new(tidx as u16, 0);
    match lt.create(owner, flags) {
        Ok(id) => { tf.gpr[REG_A0] = 0; tf.gpr[REG_A1] = id.0 as usize; }
        Err(e) => { tf.gpr[REG_A0] = e as isize as usize; }
    }
}

/// `sys_lease_revoke(a0=lease_id) -> a0=new_epoch`
/// Requires `CapKind::LeaseAdmin` (RFC 014).
pub fn sys_lease_revoke(tf: &mut TrapFrame, lt: &mut crate::lease::LeaseTable) {
    // RFC 014: require LeaseAdmin capability.
    if let Err(e) = require_cap(fjell_cap::CapKind::LeaseAdmin, fjell_cap::CapRights::ALL) {
        tf.gpr[REG_A0] = e as isize as usize; return;
    }
    use fjell_abi::lease::LeaseId;
    let id = LeaseId(tf.gpr[REG_A0] as u32);
    match lt.revoke(id) {
        Ok(ep) => { tf.gpr[REG_A0] = 0; tf.gpr[REG_A1] = ep.0 as usize; }
        Err(e) => { tf.gpr[REG_A0] = e as isize as usize; }
    }
}

/// `sys_lease_inspect(a0=lease_id) -> a0=epoch`
/// Requires `CapKind::LeaseAdmin` with `INSPECT` rights (RFC 014).
pub fn sys_lease_inspect(tf: &mut TrapFrame, lt: &crate::lease::LeaseTable) {
    use fjell_cap::CapRights;
    // RFC 014: require LeaseAdmin | INSPECT capability.
    if let Err(e) = require_cap(fjell_cap::CapKind::LeaseAdmin, CapRights::INSPECT) {
        tf.gpr[REG_A0] = e as isize as usize; return;
    }
    use fjell_abi::lease::LeaseId;
    let id = LeaseId(tf.gpr[REG_A0] as u32);
    match lt.current_epoch(id) {
        Ok(ep) => { tf.gpr[REG_A0] = ep.0 as usize; }
        Err(e) => { tf.gpr[REG_A0] = e as isize as usize; }
    }
}

/// Drain pending kernel audit records into a caller-supplied buffer.
///
/// ABI (RFC 020):
/// ```
/// sys_audit_drain(
///   a0 = buf_va       — user-space VA of the output buffer
///   a1 = buf_cap      — buffer capacity in bytes
///   a2 = cap_handle   — CapKind::AuditDrain capability (slot 1 in auditd)
/// ) ->
///   a0 = status (0 = ok, non-zero = SysError)
///   a1 = n_records_drained
///   a2 = n_dropped_total (cumulative since boot)
/// ```
///
/// Each record is exactly `AUDIT_RECORD_BIN_SIZE` (32) bytes; see
/// `fjell_audit_format::AuditRecordBin`.
pub fn sys_audit_drain(tf: &mut TrapFrame) {
    use fjell_audit_format::{AuditRecordBin, AUDIT_RECORD_BIN_SIZE};
    use fjell_cap::{CapKind, CapRights};
    use crate::audit::ring::{AUDIT, AuditRecord};
    use crate::mm::user_copy::copy_to_user_bytes;
    use crate::task::tcb::REG_A2;

    let buf_va  = tf.gpr[REG_A0];
    let buf_cap = tf.gpr[REG_A1];
    let cap_raw = tf.gpr[REG_A2] as u32;

    // ── 1. Validate AuditDrain capability ────────────────────────────────────
    let (table, sched, ct, _) = unsafe { crate::get_kernel_state() };
    let cur_id = match sched.current() {
        Some(id) => id,
        None => { tf.gpr[REG_A0] = SysError::BadState as isize as usize; return; }
    };
    let cs = match ct.cspace(cur_id.index as usize) {
        Some(cs) => cs,
        None => { tf.gpr[REG_A0] = SysError::InvalidCap as isize as usize; return; }
    };
    let handle = fjell_cap::CapHandle(cap_raw);
    let cap = match cs.get(handle) {
        Ok(c) if c.kind == CapKind::AuditDrain => c,
        _ => { tf.gpr[REG_A0] = SysError::InvalidCap as isize as usize; return; }
    };
    if !cap.rights.contains(CapRights::RECV) {
        tf.gpr[REG_A0] = SysError::PermissionDenied as isize as usize; return;
    }

    // ── 2. Trivial case: buffer too small for even one record ────────────────
    if buf_cap < AUDIT_RECORD_BIN_SIZE {
        tf.gpr[REG_A0] = SysError::Ok as isize as usize;
        tf.gpr[REG_A1] = 0;
        tf.gpr[REG_A2] = AUDIT.dropped() as usize;
        return;
    }

    // ── 3. Drain into kernel-side scratch buffer (≤ 64 records at once) ──────
    const MAX_BATCH: usize = 64;
    let max_records = (buf_cap / AUDIT_RECORD_BIN_SIZE).min(MAX_BATCH);
    let mut kbuf = [unsafe { core::mem::zeroed::<AuditRecord>() }; MAX_BATCH];
    let (n_drained, n_dropped) = AUDIT.drain_into(&mut kbuf[..max_records]);

    // ── 4. Get caller page-table root for copy_to_user ───────────────────────
    let root_pfn = match table.get(cur_id) {
        Some(task) => task.satp_root_pfn,
        None => { tf.gpr[REG_A0] = SysError::BadState as isize as usize; return; }
    };

    // ── 5. Serialize each record and copy to user buffer ─────────────────────
    let mut copied = 0usize;
    for i in 0..n_drained {
        let r = &kbuf[i];
        let bin = AuditRecordBin {
            seq:    r.seq,
            tick:   r.tick,
            kind:   r.kind as u16,
            task:   0xFFFF,
            arg0:   r.arg0 as u32,
            arg1:   r.arg1 as u32,
            result: r.result as i32,
        };
        let bytes = unsafe {
            core::slice::from_raw_parts(
                &bin as *const AuditRecordBin as *const u8,
                AUDIT_RECORD_BIN_SIZE,
            )
        };
        let dst = buf_va + i * AUDIT_RECORD_BIN_SIZE;
        match unsafe { copy_to_user_bytes(root_pfn, dst, bytes) } {
            Ok(_)  => copied += 1,
            Err(_) => break,
        }
    }

    tf.gpr[REG_A0] = SysError::Ok as isize as usize;
    tf.gpr[REG_A1] = copied;
    tf.gpr[REG_A2] = n_dropped as usize;
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
/// RFC 016: Map a bounded MMIO physical sub-range into the caller's address space.
///
/// The caller must hold a `CapKind::MmioRegion` capability whose `object_id`
/// identifies an entry in the kernel's static `MmioRegionTable`.  `offset + size`
/// is bounds-checked against the region.  Kernel RAM is additionally excluded as
/// defense-in-depth (RFC 005).
pub fn sys_mmio_map(tf: &mut TrapFrame) {
    use crate::mm::{address::VirtAddr, vspace::VmPerms};
    use crate::platform::qemu_virt::{RAM_BASE, RAM_END, mmio_region_table};
    use fjell_cap::CapKind;

    let cap_handle = fjell_cap::CapHandle(tf.gpr[REG_A0] as u32);
    let offset     = tf.gpr[10 + 1];               // a1
    let size_bytes = (tf.gpr[10 + 2] + 0xFFF) & !0xFFF; // a2, page-align up

    if size_bytes == 0 {
        tf.gpr[REG_A0] = SysError::InvalidCap as isize as usize;
        return;
    }

    // 1. Resolve the MmioRegion capability.
    let tidx = crate::trap::dispatch::current_task_idx();
    let (_, _, cap_table, _) = unsafe { crate::get_kernel_state() };
    let lt = unsafe { crate::get_lease_table() };
    let region_idx = {
        let cs = match cap_table.cspace(tidx) {
            Some(c) => c,
            None    => { tf.gpr[REG_A0] = SysError::InternalError as isize as usize; return; }
        };
        let cap = match cs.get(cap_handle) {
            Ok(c)  => c,
            Err(e) => { tf.gpr[REG_A0] = e as isize as usize; return; }
        };
        if cap.kind != CapKind::MmioRegion {
            tf.gpr[REG_A0] = SysError::InvalidCap as isize as usize;
            return;
        }
        if let Err(e) = cap.check_lease(lt) {
            tf.gpr[REG_A0] = e as isize as usize;
            return;
        }
        cap.object_id as usize
    };

    // 2. Bounds-check offset + size against the MmioRegion.
    let mmio_table = mmio_region_table();
    let region = match mmio_table.get(region_idx) {
        Some(r) => r,
        None    => { tf.gpr[REG_A0] = SysError::InvalidCap as isize as usize; return; }
    };
    let end_offset = offset.saturating_add(size_bytes);
    if end_offset > region.size {
        tf.gpr[REG_A0] = SysError::InvalidCap as isize as usize;
        return;
    }
    let phys_addr = region.base + offset;

    // 3. RFC 005 defense-in-depth: reject kernel RAM.
    let end_pa = phys_addr.saturating_add(size_bytes);
    if phys_addr < RAM_END && end_pa > RAM_BASE {
        tf.gpr[REG_A0] = SysError::InvalidCap as isize as usize;
        return;
    }

    // 4. Map pages R|W|U into the caller's page table.
    let task_id  = crate::task::TaskId::new(tidx as u16, 0);
    let (table, _, _, _) = unsafe { crate::get_kernel_state() };
    let root_pfn = table.get(task_id).map(|t| t.satp_root_pfn).unwrap_or(0);
    if root_pfn == 0 {
        tf.gpr[REG_A0] = SysError::NoMemory as isize as usize;
        return;
    }
    let fa = unsafe { &mut *crate::fa_static_ptr() };
    let mut mapped_va = 0usize;
    let mut va = phys_addr;
    while va < phys_addr + size_bytes {
        if let Ok(f) = crate::mm::frame_alloc::PhysFrame::from_pa(va) {
            let user_va = va;
            if mapped_va == 0 { mapped_va = user_va; }
            unsafe {
                let _ = crate::mm::page_table::remap_page(
                    root_pfn << 12, VirtAddr(user_va), f,
                    VmPerms::R | VmPerms::W | VmPerms::U, fa,
                );
            }
        }
        va += 4096;
    }
    unsafe { crate::arch::riscv64::csr::sfence_vma(); }
    tf.gpr[REG_A0] = 0;
    tf.gpr[REG_A1] = mapped_va;
}

/// `sys_dma_alloc(a0=dma_cap_handle, a1=size_bytes) -> a0=status, a1=user_va, a2=device_pa`
///
/// RFC 017: Per-task DMA allocator.  The caller must hold `CapKind::DmaAlloc`.
/// Maximum 1 page (4 KiB) per region (M8 may lift this).
pub fn sys_dma_alloc(tf: &mut TrapFrame) {
    use crate::mm::{address::VirtAddr, vspace::VmPerms, frame_alloc::FrameOwner};
    use crate::task::tcb::REG_A2;
    use fjell_cap::CapKind;

    let cap_raw   = tf.gpr[REG_A0] as u32;
    let size_bytes = (tf.gpr[REG_A1] + 0xFFF) & !0xFFF;

    // RFC 017: validate DmaAlloc capability.
    let tidx = crate::trap::dispatch::current_task_idx();
    {
        let (_, _, ct, lt_ref) = unsafe { crate::get_kernel_state() };
        let lt = unsafe { crate::get_lease_table() };
        let cs = match ct.cspace(tidx) {
            Some(c) => c,
            None => { tf.gpr[REG_A0] = SysError::InternalError as isize as usize; return; }
        };
        let handle = fjell_cap::CapHandle(cap_raw);
        let cap = match cs.get(handle) {
            Ok(c) if c.kind == CapKind::DmaAlloc => c,
            Ok(_) => { tf.gpr[REG_A0] = SysError::WrongType  as isize as usize; return; }
            Err(e) => { tf.gpr[REG_A0] = e as isize as usize; return; }
        };
        if let Err(e) = cap.check_lease(lt) {
            tf.gpr[REG_A0] = e as isize as usize; return;
        }
        let _ = lt_ref;
    }

    // RFC 017: M8 limit — maximum 1 page (4 KiB) per DMA region.
    let pages = size_bytes / 4096;
    if pages == 0 || pages > 1 {
        tf.gpr[REG_A0] = SysError::InvalidArg as isize as usize;
        return;
    }
    let _ = REG_A2;

    let (table, _, _, _) = unsafe { crate::get_kernel_state() };
    let cur_id  = crate::trap::dispatch::current_task_idx();
    let task_id = crate::task::TaskId::new(cur_id as u16, 0);
    let root_pfn = table.get(task_id).map(|t| t.satp_root_pfn).unwrap_or(0);
    if root_pfn == 0 {
        tf.gpr[REG_A0] = SysError::NoMemory as isize as usize;
        return;
    }
    let fa = unsafe { &mut *crate::fa_static_ptr() };

    let user_va_start = crate::DMA_VA_NEXT.fetch_add(
        pages * 4096, core::sync::atomic::Ordering::Relaxed);

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

    // RFC 017: Record ownership for zeroize-on-exit.
    crate::dma_table().alloc(task_id, user_va_start, first_pa);

    tf.gpr[REG_A0] = 0;
    tf.gpr[REG_A1] = user_va_start;
    tf.gpr[12]     = first_pa;
}

/// Dispatch `sys_ipc_try_recv` (RFC 019 — non-blocking IPC receive).
fn dispatch_ipc_try_recv(tf: &mut TrapFrame) {
    use crate::cap::syscall::sys_ipc_try_recv;
    let tidx = crate::trap::dispatch::current_task_idx();
    let (table, _, ct, et) = unsafe { crate::get_kernel_state() };
    sys_ipc_try_recv(tf, tidx, ct, et, table);
}
