//! Rust trap dispatcher — called from the assembly entry stub.
//!
//! # Architecture
//! The assembly stub calls `trap_dispatch(tf)` where `tf` points to the
//! saved registers of the task that just trapped.  `trap_dispatch` handles
//! the trap and returns a (possibly different) `*mut TrapFrame` pointer.
//! The assembly stub then restores that frame and executes `sret`, entering
//! the next (or same) task's user mode.
//!
//! This means ALL scheduling decisions happen inside `trap_dispatch`; there
//! is no separate "run loop" in Rust that srets to user mode repeatedly.

use crate::{
    arch::riscv64::trap::{decode_trap, TrapKind},
    audit::ring::{AuditKindInternal, AUDIT},
    task::{
        scheduler::PRIORITY_IDLE,
        tcb::{FaultCause, TaskState, TrapFrame},
        TaskId,
    },
};
use super::{fault::handle_user_fault, syscall::handle_syscall};

// ── Shared kernel state accessed from trap_dispatch ───────────────────────────
// These are the same KS statics defined in main.rs, accessed via extern.
// For M2, we use a module-local accessor pattern.

/// Transfer control to the first user task.
///
/// Restores `TrapFrame` registers and executes `sret` to enter user mode.
/// This function never returns; all subsequent scheduling happens in
/// `trap_dispatch`.
///
/// `sscratch` must already point to the static `TRAP_SCRATCH` record
/// (set up in `kmain` before calling this function).
///
/// # Safety
/// - `tf.sepc` must be a canonical user-space VA.
/// - `tf.sstatus.SPP` = 0 (U-mode), `SPIE` = 1.
/// - `sscratch` must point to the static `TRAP_SCRATCH`.
#[cfg(target_arch = "riscv64")]
// SAFETY: category=raw-pointer-deref trap frame pointer is valid for the duration of the handler; no aliasing with task state.
pub unsafe fn first_entry(tf: &TrapFrame) -> ! {
    // Pin tf to a0 (x10).  The load sequence restores every register in order;
    // if the compiler chose s3 (x19) as the base, `ld x19, 19*8(s3)` would
    // overwrite the base with TrapFrame.gpr[19] (often 0), causing the next
    // load to fault.  With a0 as the base, only x10 is clobbered — and that
    // happens on the very last load, immediately before sret.
    // SAFETY: category=csr-asm trap frame pointer is valid for the duration of the handler; no aliasing with task state.
    unsafe {
        core::arch::asm!(
            "ld   t0, {sstatus_off}(a0)",
            "csrw sstatus, t0",
            "ld   t0, {sepc_off}(a0)",
            "csrw sepc,    t0",
            "ld   x1,   1*8(a0)",
            "ld   x2,   2*8(a0)",
            "ld   x3,   3*8(a0)",
            "ld   x4,   4*8(a0)",
            "ld   x5,   5*8(a0)",
            "ld   x6,   6*8(a0)",
            "ld   x7,   7*8(a0)",
            "ld   x8,   8*8(a0)",
            "ld   x9,   9*8(a0)",
            // x10 = a0 (base) — restored last
            "ld   x11, 11*8(a0)",
            "ld   x12, 12*8(a0)",
            "ld   x13, 13*8(a0)",
            "ld   x14, 14*8(a0)",
            "ld   x15, 15*8(a0)",
            "ld   x16, 16*8(a0)",
            "ld   x17, 17*8(a0)",
            "ld   x18, 18*8(a0)",
            "ld   x19, 19*8(a0)",
            "ld   x20, 20*8(a0)",
            "ld   x21, 21*8(a0)",
            "ld   x22, 22*8(a0)",
            "ld   x23, 23*8(a0)",
            "ld   x24, 24*8(a0)",
            "ld   x25, 25*8(a0)",
            "ld   x26, 26*8(a0)",
            "ld   x27, 27*8(a0)",
            "ld   x28, 28*8(a0)",
            "ld   x29, 29*8(a0)",
            "ld   x30, 30*8(a0)",
            "ld   x31, 31*8(a0)",
            "ld   x10, 10*8(a0)",   // a0 last — clobbers base, sret follows
            "sret",
            in("a0") tf,            // force base into a0 to prevent self-clobber
            sstatus_off = const 32 * 8,
            sepc_off    = const 33 * 8,
            options(noreturn),
        );
    }
}

/// Main trap dispatch function.
///
/// Called from `supervisor_trap_entry` with a pointer to the TrapFrame of
/// the task that just trapped.  Returns the TrapFrame pointer for the next
/// task to run (may be the same task after a yield, or a different task).
///
/// The assembly stub restores the returned TrapFrame and executes `sret`.
///
/// # Safety
/// - `tf` must point to the current task's valid, fully-saved `TrapFrame`.
/// - Must be called from the assembly trap entry with a valid kernel stack.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn trap_dispatch(tf: *mut TrapFrame) -> *mut TrapFrame {
    // SAFETY: category=csr-asm tf is provided by the assembly trap entry stub.
    let tf_ref = unsafe { &mut *tf };
    let scause = tf_ref.scause;

    match decode_trap(scause) {
        TrapKind::UserEcall    => handle_syscall(tf_ref),
        TrapKind::SupervisorTimer => handle_timer(),
        TrapKind::InstructionPageFault =>
            handle_user_fault(tf_ref, FaultCause::InstructionPageFault),
        TrapKind::LoadPageFault =>
            handle_user_fault(tf_ref, FaultCause::LoadPageFault),
        TrapKind::StorePageFault =>
            handle_user_fault(tf_ref, FaultCause::StorePageFault),
        TrapKind::IllegalInstruction =>
            handle_user_fault(tf_ref, FaultCause::IllegalInstruction),
        TrapKind::Other(cause) =>
            handle_unhandled(tf_ref, cause),
    }

    // Schedule the next task and return its TrapFrame pointer.
    schedule_next(tf)
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn handle_timer() {
    #[cfg(target_arch = "riscv64")]
    // SAFETY: category=page-table-mutation CLINT MMIO; single hart; no concurrent access.
    unsafe { crate::arch::riscv64::timer::schedule_next_tick() };
    // RFC 037: mark this as a timer preemption (distinct from voluntary yield).
    TIMER_PREEMPTED.store(true);
    super::syscall::request_yield();
}

// ── RFC 037: timer-preemption flag ───────────────────────────────────────────

/// Set when a timer interrupt fires; cleared in `schedule_next`.
///
/// Distinguishes involuntary timer preemption from voluntary `sys_yield` so
/// the scheduler can track per-task quantum violations.
static TIMER_PREEMPTED: super::syscall::Flag = super::syscall::Flag::new();

fn take_timer_preempted() -> bool { TIMER_PREEMPTED.take() }

fn handle_unhandled(tf: &mut TrapFrame, cause: usize) {
    let from_user = tf.sstatus & (1 << 8) == 0; // SPP bit
    if from_user {
        handle_user_fault(tf, FaultCause::KernelRejectedReturn);
    } else {
        panic!("kernel trap: scause={:#x} sepc={:#x}", cause, tf.sepc);
    }
}

/// Pick the next task and return its TrapFrame pointer.
///
/// Handles yield/exit/fault state transitions, then asks the scheduler for
/// the next runnable task.  Updates the sscratch record so the trap entry
/// points at the new task's TrapFrame.
fn schedule_next(current_tf: *mut TrapFrame) -> *mut TrapFrame {
    // Access global kernel state.
    // SAFETY: category=kernel-global-mutable single-hart M2/M3; initialised in kmain before first trap.
    let (table, sched, _ct, _et) = unsafe { crate::get_kernel_state() };

    let current_id = sched.current();

    // Apply trap result to the current task.
    if let Some(id) = current_id {
        if let Some(task) = table.get_mut(id) {
            if let Some(code) = super::syscall::take_exit() {
                let _label = task_label(id);
                // RFC 017: zeroize and release DMA regions before marking exited.
                crate::dma_table().release_task(id);
                // RFC 033: lifecycle revoke on task exit.
                // SAFETY: category=kernel-global-mutable trap frame pointer is valid for the duration of the handler; no aliasing with task state.
                let lt = unsafe { crate::get_lease_table() };
                lt.revoke_owned_by(id);
                task.state = TaskState::Exited(code);
                task.accounting.quantum_violations = 0;
                AUDIT.lock_free_append(AuditKindInternal::TaskExit, code as usize, 0, 0);
                sched.on_exit();
                check_smoke_pass(table);
            } else if let Some(fault) = super::fault::take_fault() {
                let label = task_label(id);
                // RISC-V ABI: x14=a4, x29=t4, x30=t5 — the registers used
                // by the LBU string-print loop at the observed fault sites.
                crate::kprintln!("[task#{} {}]: fault({:?}) sepc={:#x} stval={:#x}", id.index, label, fault.cause, fault.sepc, fault.stval);
                // RFC 017: also zeroize DMA on fault.
                crate::dma_table().release_task(id);
                // RFC 033: lifecycle revoke on task fault.
                // SAFETY: category=kernel-global-mutable trap frame pointer is valid for the duration of the handler; no aliasing with task state.
                let lt = unsafe { crate::get_lease_table() };
                lt.revoke_owned_by(id);
                task.state = TaskState::Faulted(fault);
                task.accounting.quantum_violations = 0;
                sched.on_fault();
                check_smoke_pass(table);
            } else if super::syscall::take_yield() {
                let _label = task_label(id);
                task.state = TaskState::Runnable;
                // Voluntary yield: reset quantum violation counter (RFC 037).
                task.accounting.quantum_violations = 0;
                AUDIT.lock_free_append(
                    AuditKindInternal::TaskSwitch, id.index as usize, 0, 0);
                let prio = task.priority;
                sched.on_yield(id, prio);
            } else {
                // Timer preempt or spurious: re-enqueue current.
                if matches!(task.state, TaskState::Running) {
                    task.state = TaskState::Runnable;
                    let prio = task.priority;
                    // RFC 037: track timer preemptions.
                    if take_timer_preempted() {
                        task.accounting.quantum_violations += 1;
                        if task.accounting.quantum_violations
                            >= crate::task::tcb::QUANTUM_VIOLATION_THRESHOLD
                        {
                            AUDIT.lock_free_append(
                                AuditKindInternal::TaskQuantumExceeded,
                                id.index as usize,
                                task.accounting.quantum_violations as usize,
                                0,
                            );
                        }
                    }
                    sched.enqueue_runnable(id, prio);
                }
            }
        }
    }

    // Pick next task.
    let next_id = sched.choose_next();
    sched.set_current(next_id);

    // Idle: wfi until next interrupt.
    let is_idle = table.get(next_id)
        .map(|t| t.priority == PRIORITY_IDLE)
        .unwrap_or(false);
    if is_idle {
        // SAFETY: category=kernel-global-mutable interrupts enabled after trap init.
        #[cfg(target_arch = "riscv64")]
        unsafe { crate::arch::riscv64::asm::wfi() };
        // After wfi returns (interrupt arrived), the trap entry will call
        // trap_dispatch again.  Return current_tf as a placeholder (it will
        // be overwritten by the next trap_dispatch call).
        return current_tf;
    }


    // Mark next task as running and switch to its address space.
    let next_tf = if let Some(task) = table.get_mut(next_id) {
        // Safety guard: Exited or Faulted tasks must never be dispatched.
        // If one ends up here it means something enqueued it incorrectly; skip
        // it and fall back to current_tf so the kernel stays alive.
        if matches!(task.state, TaskState::Exited(_) | TaskState::Faulted(_)) {
            crate::kprintln!("[sched] BUG: dispatched dead task #{} {} state={:?}",
                next_id.index, task_label(next_id), task.state);
            return current_tf;
        }
        task.state = TaskState::Running;
        task.accounting.run_count += 1;

        // Switch satp to the next task's root page table.
        // For the idle task (satp_root_pfn = 0) keep the kernel mapping.
        // SAFETY: category=csr-asm satp_root_pfn was set from a valid PhysFrame.pfn during task
        // creation; sfence.vma flushes stale TLB entries for ASID 0.
        #[cfg(target_arch = "riscv64")]
        if task.satp_root_pfn != 0 {
            // SAFETY: category=csr-asm trap frame pointer is valid for the duration of the handler; no aliasing with task state.
            unsafe { crate::arch::riscv64::satp::enable_sv39(task.satp_root_pfn) };
        }

        &mut task.trap_frame as *mut TrapFrame
    } else {
        current_tf
    };

    // Update TRAP_SCRATCH[1] to point at the next task's TrapFrame so the
    // trap entry stub saves/restores the correct registers next time.
    // TRAP_SCRATCH[0] (kernel sp) stays constant — always the boot stack top.
    //
    // Also explicitly restore sscratch to &TRAP_SCRATCH[0].  The trap entry
    // does `csrrw t6, sscratch, t6` (swap t6 and sscratch), which leaves
    // sscratch = user's t6 after entry.  If we don't restore it here, the
    // *next* trap would load scratch_addr=user_t6 (often 0) and fault on
    // `ld sp, 0(t6)`.
    //
    // SAFETY: category=kernel-global-mutable TRAP_SCRATCH is static and was initialised in kmain before any
    // trap fired.  Single-hart M3; no concurrent access.
    #[cfg(target_arch = "riscv64")]
    unsafe {
        let s = &mut *crate::TRAP_SCRATCH.0.get();
        s[1] = next_tf as usize;
        crate::arch::riscv64::csr::write_sscratch(s.as_ptr() as usize);
    }

    next_tf
}

fn task_label(id: crate::task::TaskId) -> &'static str {
    // Slots confirmed from live logs (spawn order depends on ImageId ordering
    // in fjell-abi, which may differ across builds).
    // Kernel-side labels are best-effort for diagnostics only.
    match id.index {
        0  => "idle",
        1  => "init",
        2  => "configd",
        3  => "cap-broker",
        4  => "auditd",
        5  => "svc-manager",
        6  => "sample",
        7  => "neg-test",
        8  => "sem-stream",
        9  => "proxy-text",
        10 => "devmgr",
        11 => "virtio-blk",
        12 => "storaged",
        13 => "bootctl",
        14 => "upgraded",
        _  => "task",
    }
}

fn check_smoke_pass(table: &crate::task::tcb::TaskTable) {
    // Pass conditions by milestone.
    // init (slot 1) orchestrates M1-M7 and exits after those complete.
    // upgraded (slot 14) orchestrates M8 and exits after M8 completes.
    let exited_ok = |idx: u16| {
        table.get(TaskId::new(idx, 0))
             .map(|t| matches!(t.state, TaskState::Exited(0)))
             .unwrap_or(false)
    };
    let done = |idx: u16| {
        table.get(TaskId::new(idx, 0))
             .map(|t| matches!(t.state, TaskState::Exited(_) | TaskState::Faulted(_)))
             .unwrap_or(true)
    };

    // Emit milestone markers atomically from the kernel so they are never
    // garbled by concurrent user-space UART writes.
    //
    // Spawn order: devmgr(10) upgraded(14) syncd(19) netd(21)
    if exited_ok(10) { crate::kprintln!("TEST:V0.5-PLATFORM:PASS"); }
    if exited_ok(19) { crate::kprintln!("TEST:V0.7-SYNC:PASS"); }
    if exited_ok(21) { crate::kprintln!("TEST:V0.4-NET:PASS"); }

    // M8 pass: upgraded (slot 14) exited cleanly.
    if exited_ok(14) {
        crate::kprintln!("TEST:M8:PASS");
    } else if exited_ok(1) {
        // M7 pass: init (slot 1) exited cleanly but M8 not yet done.
        crate::kprintln!("TEST:M7:PASS");
    } else if done(1) {
        crate::kprintln!("TEST:M7:FAIL (init did not exit cleanly)");
    }
}

/// Return the index of the currently running task (used by M4 syscall dispatch).
///
/// Returns 0 (idle) if no task is currently scheduled.
pub fn current_task_idx() -> usize {
    // SAFETY: category=kernel-global-mutable trap frame pointer is valid for the duration of the handler; no aliasing with task state.
    let (_, sched, _, _) = unsafe { crate::get_kernel_state() };
    sched.current().map(|id| id.index as usize).unwrap_or(0)
}
