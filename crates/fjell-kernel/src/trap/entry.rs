//! Assembly trap-entry stub and `stvec` installation.
//!
//! sscratch layout (set by main.rs before first_entry, updated by schedule_next):
//!   [0]   kernel sp  (boot stack top)
//!   [8]   &TrapFrame (current task's trap frame)
//!   [16]  temp user sp  save (written on entry, read back when saving gpr[2])
//!   [24]  temp user t6  save (RFC 001: added to fix gpr[31] correctness)

use crate::arch::riscv64::csr::write_stvec;

/// Install `supervisor_trap_entry` into `stvec` (direct mode).
///
/// # Safety
/// Must be called after sscratch is initialised and before any user-mode
/// transition or S-mode interrupt enable.
// SAFETY: category=csr-asm invariants upheld by the surrounding context; see module documentation.
pub unsafe fn init_trap() {
    // SAFETY: category=csr-asm supervisor_trap_entry is 4-byte aligned; direct mode (mode=0).
    unsafe { write_stvec(supervisor_trap_entry as *const () as usize) };
}

/// Naked trap entry: save registers → call trap_dispatch → restore → sret.
///
/// sscratch[0] = kernel sp, sscratch[8] = &TrapFrame,
/// sscratch[16] = temp user sp, sscratch[24] = temp user t6.
///
/// # RFC 001 — corrected save sequence for t5 (x30) and t6 (x31)
///
/// Entry state: t6 is about to be clobbered by `csrrw t6, sscratch, t6`.
/// We must save the true user_t6 into scratch[3] BEFORE that swap.
///
/// Step-by-step:
///   1. csrrw t6, sscratch, t6   → t6 = scratch_addr,  sscratch = user_t6
///   2. csrr  t5, sscratch       → t5 = user_t6
///   3. sd    t5, 24(t6)         → scratch[3] = user_t6  (saved before anything clobbers it)
///   4. sd    sp, 16(t6)         → scratch[2] = user_sp
///   5. ld    sp, 0(t6)          → sp = kernel_sp
///   6. csrw  sscratch, t6       → sscratch = scratch_addr  (restored for next entry)
///   7. ld    t6, 8(t6)          → t6 = &TrapFrame
///   8. sd    x30, 30*8(t6)      → gpr[30] = true user_t5  ✓  (x30 is still live here)
///   9. … save x1..x29 (x30 already saved) …
///  10. Retrieve user_sp from scratch[2]; save as gpr[2]       ✓
///  11. Retrieve user_t6 from scratch[3]; save as gpr[31]      ✓
#[cfg(target_arch = "riscv64")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
unsafe extern "C" fn supervisor_trap_entry() {
    core::arch::naked_asm!(
        // ── Save phase ───────────────────────────────────────────────────
        // Step 1: t6 ↔ sscratch swap.  After: t6=scratch_addr, sscratch=user_t6.
        "csrrw  t6, sscratch, t6",

        // Step 2-3: save true user_t6 into scratch[3] before anything clobbers it.
        "csrr   t5, sscratch",       // t5 = user_t6  (via sscratch)
        "sd     t5, 24(t6)",         // scratch[3] = user_t6

        // Step 4: save user_sp into scratch[2] before we overwrite sp.
        "sd     sp, 16(t6)",         // scratch[2] = user_sp

        // Step 5: load kernel sp.
        "ld     sp, 0(t6)",

        // Step 6: restore sscratch = scratch_addr for the next trap entry.
        "csrw   sscratch, t6",

        // Step 7: load TrapFrame ptr from scratch[1].
        "ld     t6, 8(t6)",          // t6 = &TrapFrame

        // Step 8: save true user_t5 (x30) IMMEDIATELY while x30 is still live.
        // (all subsequent csrr/ld instructions may overwrite x30/t5)
        "sd     x30, 30*8(t6)",      // gpr[30] = true user_t5  ✓

        // Save x1..x29 (x30 already saved above; x31 saved below).
        "sd     x1,   1*8(t6)",
        // Step 10: gpr[2] = user_sp — retrieved from scratch[2] via sscratch.
        "csrr   t5, sscratch",       // t5 = scratch_addr
        "ld     t5, 16(t5)",         // t5 = scratch[2] = user_sp
        "sd     t5,  2*8(t6)",       // gpr[2] = user_sp  ✓
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
        // x30 already saved at step 8 above.
        // Step 11: gpr[31] = true user_t6 — retrieved from scratch[3].
        "csrr   t5, sscratch",       // t5 = scratch_addr
        "ld     t5, 24(t5)",         // t5 = scratch[3] = user_t6
        "sd     t5, 31*8(t6)",       // gpr[31] = user_t6  ✓
        // Save sstatus, sepc, scause, stval.
        "csrr   t5, sstatus",  "sd t5, 32*8(t6)",
        "csrr   t5, sepc",     "sd t5, 33*8(t6)",
        "csrr   t5, scause",   "sd t5, 34*8(t6)",
        "csrr   t5, stval",    "sd t5, 35*8(t6)",

        // ── Dispatch ─────────────────────────────────────────────────────
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
