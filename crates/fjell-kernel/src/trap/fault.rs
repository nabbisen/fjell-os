//! User-mode fault containment.
//!
//! Invariants (TRAP-*):
//!   TRAP-003  User page fault → TaskState::Faulted; no kernel panic.
//!   TRAP-004  Kernel page fault → panic (legitimate).

use crate::{
    audit::ring::{AuditKindInternal, AUDIT},
    task::tcb::{FaultCause, FaultInfo, TrapFrame},
};

/// Handle a user-mode fault by marking the current task as faulted.
///
/// Does not panic — the fault is contained within the faulted task.
/// The kernel run-loop detects the `Faulted` state and switches to
/// another task or idle.
pub fn handle_user_fault(tf: &TrapFrame, cause: FaultCause) {
    // Verify the trap came from user mode (SPP bit in sstatus must be 0).
    // If SPP == 1 the fault came from S-mode — that is a kernel bug.
    // TRAP-004: kernel faults are allowed to panic.
    let from_user = tf.sstatus & (1 << 8) == 0;
    if !from_user {
        panic!(
            "kernel-mode fault: cause={:?}, sepc={:#x}, stval={:#x}",
            cause, tf.sepc, tf.stval
        );
    }

    let info = FaultInfo { cause, sepc: tf.sepc, stval: tf.stval };

    // Record in audit ring.
    AUDIT.lock_free_append(
        AuditKindInternal::TaskFault,
        tf.sepc,
        tf.stval,
        0,
    );

    // Signal the run-loop to mark the current task Faulted.
    FAULT_INFO.store(Some(info));
}

// ── Fault signal to run-loop ──────────────────────────────────────────────────

pub(crate) struct FaultCell(core::cell::Cell<Option<FaultInfo>>);
// SAFETY: single-hart M2.
unsafe impl Sync for FaultCell {}
impl FaultCell {
    const fn new() -> Self { FaultCell(core::cell::Cell::new(None)) }
    fn store(&self, v: Option<FaultInfo>) { self.0.set(v); }
    fn take(&self) -> Option<FaultInfo> { let v = self.0.get(); self.0.set(None); v }
}

pub(crate) static FAULT_INFO: FaultCell = FaultCell::new();

/// Called by the kernel run-loop after each trap to check for user faults.
pub fn take_fault() -> Option<FaultInfo> {
    FAULT_INFO.take()
}
