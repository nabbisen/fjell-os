//! Assembly trap-entry stub and `stvec` installation.
//!
//! sscratch layout (set by main.rs before first_entry, updated by schedule_next):
//!   [0]  kernel sp  (boot stack top)
//!   [8]  &TrapFrame (current task's trap frame)

use crate::arch::riscv64::csr::write_stvec;

/// Install `supervisor_trap_entry` into `stvec` (direct mode).
///
/// # Safety
/// Must be called after sscratch is initialised and before any user-mode
/// transition or S-mode interrupt enable.
pub unsafe fn init_trap() {
    // SAFETY: supervisor_trap_entry is 4-byte aligned; direct mode (mode=0).
    unsafe { write_stvec(supervisor_trap_entry as *const () as usize) };
}

/// Naked trap entry: save registers → call trap_dispatch → restore → sret.
///
/// sscratch[0] = kernel sp, sscratch[8] = &TrapFrame.
/// After csrrw t6, sscratch, t6:
///   t6       = scratch_addr   (was sscratch)
///   sscratch = old_t6         (x31, saved later)
#[cfg(target_arch = "riscv64")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
unsafe extern "C" fn supervisor_trap_entry() {
    core::arch::naked_asm!(
        // ── Save phase ───────────────────────────────────────────────────
        // t6 ↔ sscratch: t6 = scratch_addr, sscratch = old_t6.
        "csrrw  t6, sscratch, t6",
        // Load kernel sp from scratch[0].
        "ld     sp, 0(t6)",
        // Load TrapFrame ptr from scratch[8].
        "ld     t6, 8(t6)",          // t6 = &TrapFrame

        // Save x1..x30 (x0 is always zero; x31/t6 saved below).
        "sd     x1,   1*8(t6)",
        "sd     x2,   2*8(t6)",
        "sd     x3,   3*8(t6)",
        "sd     x4,   4*8(t6)",
        "sd     x5,   5*8(t6)",
        "sd     x6,   6*8(t6)",
        "sd     x7,   7*8(t6)",
        "sd     x8,   8*8(t6)",
        "sd     x9,   9*8(t6)",
        "sd     x10, 10*8(t6)",
        "sd     x11, 11*8(t6)",
        "sd     x12, 12*8(t6)",
        "sd     x13, 13*8(t6)",
        "sd     x14, 14*8(t6)",
        "sd     x15, 15*8(t6)",
        "sd     x16, 16*8(t6)",
        "sd     x17, 17*8(t6)",
        "sd     x18, 18*8(t6)",
        "sd     x19, 19*8(t6)",
        "sd     x20, 20*8(t6)",
        "sd     x21, 21*8(t6)",
        "sd     x22, 22*8(t6)",
        "sd     x23, 23*8(t6)",
        "sd     x24, 24*8(t6)",
        "sd     x25, 25*8(t6)",
        "sd     x26, 26*8(t6)",
        "sd     x27, 27*8(t6)",
        "sd     x28, 28*8(t6)",
        "sd     x29, 29*8(t6)",
        "sd     x30, 30*8(t6)",
        // Recover original t6 (x31) from sscratch, then save it.
        "csrr   t5, sscratch",
        "sd     t5, 31*8(t6)",
        // Save sstatus, sepc, scause, stval.
        "csrr   t5, sstatus",  "sd t5, 32*8(t6)",
        "csrr   t5, sepc",     "sd t5, 33*8(t6)",
        "csrr   t5, scause",   "sd t5, 34*8(t6)",
        "csrr   t5, stval",    "sd t5, 35*8(t6)",

        // ── Dispatch ─────────────────────────────────────────────────────
        // Call trap_dispatch(tf) → returns *mut TrapFrame for next task.
        "mv     a0, t6",
        "call   trap_dispatch",
        // a0 = next TrapFrame ptr.

        // ── Restore phase ─────────────────────────────────────────────────
        "ld     t5, 32*8(a0)",  "csrw sstatus, t5",
        "ld     t5, 33*8(a0)",  "csrw sepc,    t5",

        "ld     x1,   1*8(a0)",
        "ld     x2,   2*8(a0)",
        "ld     x3,   3*8(a0)",
        "ld     x4,   4*8(a0)",
        "ld     x5,   5*8(a0)",
        "ld     x6,   6*8(a0)",
        "ld     x7,   7*8(a0)",
        "ld     x8,   8*8(a0)",
        "ld     x9,   9*8(a0)",
        "ld     x11, 11*8(a0)",
        "ld     x12, 12*8(a0)",
        "ld     x13, 13*8(a0)",
        "ld     x14, 14*8(a0)",
        "ld     x15, 15*8(a0)",
        "ld     x16, 16*8(a0)",
        "ld     x17, 17*8(a0)",
        "ld     x18, 18*8(a0)",
        "ld     x19, 19*8(a0)",
        "ld     x20, 20*8(a0)",
        "ld     x21, 21*8(a0)",
        "ld     x22, 22*8(a0)",
        "ld     x23, 23*8(a0)",
        "ld     x24, 24*8(a0)",
        "ld     x25, 25*8(a0)",
        "ld     x26, 26*8(a0)",
        "ld     x27, 27*8(a0)",
        "ld     x28, 28*8(a0)",
        "ld     x29, 29*8(a0)",
        "ld     x30, 30*8(a0)",
        "ld     x31, 31*8(a0)",
        "ld     x10, 10*8(a0)",  // a0 (x10) last
        "sret",
    );
}

/// Host stub.
#[cfg(not(target_arch = "riscv64"))]
unsafe extern "C" fn supervisor_trap_entry() { unimplemented!() }
