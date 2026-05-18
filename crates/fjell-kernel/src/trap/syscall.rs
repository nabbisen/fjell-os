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

// ── RFC 004: capability gating helper ────────────────────────────────────────

/// Returns `true` if the calling task holds at least one capability of the
/// given `kind` in its CSpace.
///
/// Used to gate `sys_task_spawn` (TaskCreate), `sys_task_start`/`sys_task_status`
/// (TaskControl), and `sys_lease_*` (LeaseAdmin).
fn caller_has_cap(kind: fjell_cap::CapKind) -> bool {
    use crate::trap::dispatch::current_task_idx;
    let (_, _, cap_table, _) = unsafe { crate::get_kernel_state() };
    let tidx = current_task_idx();
    let cs = match cap_table.cspace(tidx) { Some(c) => c, None => return false };
    cs.slots().iter().any(|slot| {
        slot.cap.map_or(false, |cap| cap.kind == kind)
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
    // RFC 004: require TaskCreate capability.
    if !caller_has_cap(fjell_cap::CapKind::TaskCreate) {
        tf.gpr[REG_A0] = SysError::PermissionDenied as isize as usize;
        return;
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
    // RFC 004: require TaskControl capability.
    if !caller_has_cap(fjell_cap::CapKind::TaskControl) {
        tf.gpr[REG_A0] = SysError::PermissionDenied as isize as usize;
        return;
    }
    // RFC 010: decode (index, generation) from packed u32 handle.
    let raw        = tf.gpr[REG_A0] as u32;
    let index      = (raw & 0xFFFF) as u16;
    let generation = (raw >> 16) as u16;
    let entry  = tf.gpr[10 + 1]; // a1
    let stack  = tf.gpr[10 + 2]; // a2
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
pub fn sys_task_status(tf: &mut TrapFrame, table: &crate::task::tcb::TaskTable) {
    use fjell_abi::error::SysError;
    use fjell_abi::service::TaskLifecycle;
    use crate::task::TaskId; use crate::task::tcb::TaskState;
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
    // RFC 004: require LeaseAdmin capability.
    if !caller_has_cap(fjell_cap::CapKind::LeaseAdmin) {
        tf.gpr[REG_A0] = SysError::PermissionDenied as isize as usize;
        return;
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

// ── M6 syscall handlers ───────────────────────────────────────────────────────

/// `sys_platform_info_get() -> a0=0, a1=virtio_blk_base_pa`
///
/// Scans all 8 virtio-mmio slots on QEMU virt (0x10001000..0x10008000) and
/// returns the base PA of the first slot that contains a virtio block device
/// (magic=0x74726976, version=2, device_id=2).
pub fn sys_platform_info_get(tf: &mut TrapFrame) {
    const VIRTIO_BASE:  usize = 0x1000_1000;
    const VIRTIO_SLOTS: usize = 8;
    const VIRTIO_STRIDE:usize = 0x1000;
    // Scan in reverse: QEMU assigns virtio devices from the highest slot down.
    for i in (0..VIRTIO_SLOTS).rev() {
        let base = VIRTIO_BASE + i * VIRTIO_STRIDE;
        let magic   = unsafe { core::ptr::read_volatile((base + 0x000) as *const u32) };
        let version = unsafe { core::ptr::read_volatile((base + 0x004) as *const u32) };
        let dev_id  = unsafe { core::ptr::read_volatile((base + 0x008) as *const u32) };
        // Debug: print what we read (will appear on UART)
        // Accept version 1 (legacy) and version 2 (modern).
        if magic == 0x7472_6976 && (version == 1 || version == 2) && dev_id == 2 {
            tf.gpr[REG_A0] = 0;
            tf.gpr[REG_A1] = base;
            return;
        }
    }
    // No virtio-blk found.
    tf.gpr[REG_A0] = 1; // error
    tf.gpr[REG_A1] = 0;
}

/// `sys_mmio_map(a0=phys_addr, a1=size_bytes) -> a0=status, a1=user_va`
///
/// Maps the requested MMIO range (page-aligned) into the calling task's
/// address space.  Returns the identity-mapped user VA (= phys_addr).
///
/// # Security — RFC 005
/// Requests overlapping kernel RAM (`RAM_BASE..RAM_END`) are unconditionally
/// rejected to prevent user-space from obtaining a writable mapping of kernel
/// text, data, or stack pages.
pub fn sys_mmio_map(tf: &mut TrapFrame) {
    use crate::mm::{address::VirtAddr, vspace::VmPerms};
    use crate::platform::qemu_virt::{RAM_BASE, RAM_END};
    let phys_addr  = tf.gpr[REG_A0] & !0xFFF;      // page-align down
    let size_bytes = (tf.gpr[REG_A1] + 0xFFF) & !0xFFF; // page-align up

    // RFC 005: reject any request that overlaps kernel RAM.
    let end_addr = phys_addr.saturating_add(size_bytes);
    if phys_addr < RAM_END && end_addr > RAM_BASE {
        tf.gpr[REG_A0] = SysError::InvalidCap as isize as usize;
        return;
    }

    let (table, _, _, _) = unsafe { crate::get_kernel_state() };
    let cur_id   = crate::trap::dispatch::current_task_idx();
    let root_pfn = table.get(crate::task::TaskId::new(cur_id as u16, 0))
        .map(|t| t.satp_root_pfn).unwrap_or(0);
    if root_pfn == 0 { tf.gpr[REG_A0] = SysError::InvalidCap as isize as usize; return; }
    let root_pa = root_pfn << 12;

    let fa = unsafe { &mut *crate::fa_static_ptr() };
    let pages = size_bytes / 4096;
    for pg in 0..pages {
        let pa = phys_addr + pg * 4096;
        let frame = match crate::mm::address::PhysFrame::from_pa(pa) {
            Ok(f) => f, Err(_) => { tf.gpr[REG_A0] = SysError::InvalidCap as isize as usize; return; }
        };
        unsafe {
            // Use remap_page to allow upgrading existing kernel-only mappings.
            let _ = crate::mm::page_table::remap_page(
                root_pa, VirtAddr(pa), frame,
                VmPerms::R | VmPerms::W | VmPerms::U,
                fa,
            );
        }
    }
    unsafe { crate::arch::riscv64::csr::sfence_vma(); }
    tf.gpr[REG_A0] = 0;
    tf.gpr[REG_A1] = phys_addr;
}



/// `sys_dma_alloc(a0=size_bytes) -> a0=status, a1=user_va, a2=device_pa`
///
/// RFC 007: Per-task DMA allocator.
///
/// Allocates `size_bytes` (rounded to 4 KiB pages) of kernel RAM, maps them
/// into the calling task's page table at VA 0x6000_0000+ (VPN[2]=1), and
/// returns both the user VA and the physical address for the device.
///
/// VPN[2]=1 is task-local (not shared via clone_kernel_half), so `map_page`
/// succeeds without AlreadyMapped collision.
pub fn sys_dma_alloc(tf: &mut TrapFrame) {
    use crate::mm::{address::VirtAddr, vspace::VmPerms, frame_alloc::FrameOwner};

    let size_bytes = (tf.gpr[REG_A0] + 0xFFF) & !0xFFF;
    let pages = size_bytes / 4096;
    if pages == 0 || pages > 8 {
        tf.gpr[REG_A0] = SysError::InvalidCap as isize as usize;
        return;
    }

    let (table, _, _, _) = unsafe { crate::get_kernel_state() };
    let cur_id  = crate::trap::dispatch::current_task_idx();
    let task_id = crate::task::TaskId::new(cur_id as u16, 0);
    let root_pfn = table.get(task_id).map(|t| t.satp_root_pfn).unwrap_or(0);
    if root_pfn == 0 {
        tf.gpr[REG_A0] = SysError::NoMemory as isize as usize;
        return;
    }
    let root_pa = root_pfn << 12;
    let fa = unsafe { &mut *crate::fa_static_ptr() };

    let user_va_start = crate::DMA_VA_NEXT.fetch_add(
        pages * 4096, core::sync::atomic::Ordering::Relaxed);

    let mut first_pa = 0usize;
    for pg in 0..pages {
        let frame = match fa.alloc_frame(FrameOwner::UserStack { task: task_id }) {
            Ok(f) => f,
            Err(_) => { tf.gpr[REG_A0] = SysError::NoMemory as isize as usize; return; }
        };
        if pg == 0 { first_pa = frame.pa(); }
        let user_va = user_va_start + pg * 4096;
        unsafe {
            let _ = crate::mm::page_table::map_page(
                root_pa, VirtAddr(user_va), frame,
                VmPerms::R | VmPerms::W | VmPerms::U, fa,
            );
        }
    }
    unsafe { crate::arch::riscv64::csr::sfence_vma(); }

    tf.gpr[REG_A0] = 0;
    tf.gpr[REG_A1] = user_va_start;
    tf.gpr[12]     = first_pa;
}
