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
pub unsafe fn first_entry(tf: &TrapFrame) -> ! {
    // SAFETY: tf is a fully-initialised TrapFrame.  We restore CSRs and GPRs
    // then execute sret.  The CPU enters user mode and this function diverges.
    unsafe {
        core::arch::asm!(
            "ld   t0, {sstatus_off}({tf})",
            "csrw sstatus, t0",
            "ld   t0, {sepc_off}({tf})",
            "csrw sepc,    t0",
            "ld   x1,   1*8({tf})",
            "ld   x2,   2*8({tf})",
            "ld   x3,   3*8({tf})",
            "ld   x4,   4*8({tf})",
            "ld   x5,   5*8({tf})",
            "ld   x6,   6*8({tf})",
            "ld   x7,   7*8({tf})",
            "ld   x8,   8*8({tf})",
            "ld   x9,   9*8({tf})",
            "ld   x11, 11*8({tf})",
            "ld   x12, 12*8({tf})",
            "ld   x13, 13*8({tf})",
            "ld   x14, 14*8({tf})",
            "ld   x15, 15*8({tf})",
            "ld   x16, 16*8({tf})",
            "ld   x17, 17*8({tf})",
            "ld   x18, 18*8({tf})",
            "ld   x19, 19*8({tf})",
            "ld   x20, 20*8({tf})",
            "ld   x21, 21*8({tf})",
            "ld   x22, 22*8({tf})",
            "ld   x23, 23*8({tf})",
            "ld   x24, 24*8({tf})",
            "ld   x25, 25*8({tf})",
            "ld   x26, 26*8({tf})",
            "ld   x27, 27*8({tf})",
            "ld   x28, 28*8({tf})",
            "ld   x29, 29*8({tf})",
            "ld   x30, 30*8({tf})",
            "ld   x31, 31*8({tf})",
            "ld   x10, 10*8({tf})",   // a0 last
            "sret",
            tf          = in(reg) tf,
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
    // SAFETY: tf is provided by the assembly trap entry stub.
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
    // SAFETY: CLINT MMIO; single hart; no concurrent access.
    unsafe { crate::arch::riscv64::timer::schedule_next_tick() };
    super::syscall::request_yield();
}

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
    // SAFETY: single-hart M2/M3; initialised in kmain before first trap.
    let (table, sched, _ct, _et) = unsafe { crate::get_kernel_state() };

    let current_id = sched.current();

    // Apply trap result to the current task.
    if let Some(id) = current_id {
        if let Some(task) = table.get_mut(id) {
            if let Some(code) = super::syscall::take_exit() {
                let label = task_label(id);
                crate::kprintln!("{}: exit({})", label, code);
                task.state = TaskState::Exited(code);
                AUDIT.lock_free_append(AuditKindInternal::TaskExit, code as usize, 0, 0);
                sched.on_exit();
                check_smoke_pass(table);
            } else if let Some(fault) = super::fault::take_fault() {
                let label = task_label(id);
                crate::kprintln!("{}: fault({:?})", label, fault.cause);
                task.state = TaskState::Faulted(fault);
                sched.on_fault();
                check_smoke_pass(table);
            } else if super::syscall::take_yield() {
                let label = task_label(id);
                crate::kprintln!("{}: yield", label);
                task.state = TaskState::Runnable;
                AUDIT.lock_free_append(
                    AuditKindInternal::TaskSwitch, id.index as usize, 0, 0);
                let prio = task.priority;
                sched.on_yield(id, prio);
            } else {
                // Timer preempt or spurious: re-enqueue current.
                if matches!(task.state, TaskState::Running) {
                    task.state = TaskState::Runnable;
                    let prio = task.priority;
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
        // SAFETY: interrupts enabled after trap init.
        #[cfg(target_arch = "riscv64")]
        unsafe { crate::arch::riscv64::asm::wfi() };
        // After wfi returns (interrupt arrived), the trap entry will call
        // trap_dispatch again.  Return current_tf as a placeholder (it will
        // be overwritten by the next trap_dispatch call).
        return current_tf;
    }

    // Mark next task as running.
    let next_tf = if let Some(task) = table.get_mut(next_id) {
        task.state = TaskState::Running;
        task.accounting.run_count += 1;
        &mut task.trap_frame as *mut TrapFrame
    } else {
        current_tf
    };

    // Update TRAP_SCRATCH[1] to point at the next task's TrapFrame so the
    // trap entry stub saves/restores the correct registers next time.
    // TRAP_SCRATCH[0] (kernel sp) stays constant — always the boot stack top.
    // SAFETY: TRAP_SCRATCH is static and was initialised in kmain before any
    // trap fired.  Single-hart M2; no concurrent access.
    #[cfg(target_arch = "riscv64")]
    unsafe {
        let s = &mut *crate::TRAP_SCRATCH.0.get();
        s[1] = next_tf as usize;
        // sscratch already points at TRAP_SCRATCH from kmain setup; no need
        // to call write_sscratch again unless we change the scratch address.
    }

    next_tf
}

fn task_label(id: TaskId) -> &'static str {
    match id.index {
        1 => "user0",
        2 => "user1",
        _ => "task?",
    }
}

fn check_smoke_pass(table: &crate::task::tcb::TaskTable) {
    // M3 pass condition: both user tasks reached Exited(0) via the
    // ipc_call / ipc_recv / ipc_reply flow.
    let exited_ok = |idx: u16| {
        table.get(TaskId::new(idx, 0))
             .map(|t| matches!(t.state, TaskState::Exited(0)))
             .unwrap_or(false)
    };
    let faulted_or_exited = |idx: u16| {
        table.get(TaskId::new(idx, 0))
             .map(|t| matches!(t.state, TaskState::Exited(_) | TaskState::Faulted(_)))
             .unwrap_or(true)
    };

    if exited_ok(1) && exited_ok(2) {
        crate::kprintln!("sched: idle");
        crate::kprintln!("TEST:M3:PASS");
    } else if faulted_or_exited(1) && faulted_or_exited(2) {
        // At least one task faulted — still print idle but not PASS.
        crate::kprintln!("sched: idle");
    }
}
