#![allow(dead_code)]
//! Context switch between kernel execution contexts.
//!
//! The actual register save/restore is performed in assembly.  The Rust
//! function defined here is the safe wrapper called by the scheduler.

use super::tcb::KernelContext;

/// Switch from `current` to `next` kernel context.
///
/// Saves callee-saved registers into `*current` and restores them from
/// `*next`.  Returns in the context of `next`.
///
/// # Safety
/// - Both pointers must be valid, properly aligned `KernelContext` objects.
/// - `next` must point to a context that was previously saved by this
///   function (or initialised to point to a valid kernel stack and entry).
/// - Must be called with a valid kernel stack; must not be called from an
///   interrupt handler.
#[cfg(target_arch = "riscv64")]
// SAFETY: task stack and entry point are validated during service manifest parsing.
pub unsafe fn context_switch(current: *mut KernelContext, next: *const KernelContext) {
    // SAFETY: assembly saves/restores exactly the callee-saved registers
    // listed in KernelContext.  Both pointers are valid per the caller's
    // contract.
    unsafe {
        core::arch::asm!(
            // Save current context.
            "sd ra,   0*8({cur})",
            "sd sp,   1*8({cur})",
            "sd s0,   2*8({cur})",
            "sd s1,   3*8({cur})",
            "sd s2,   4*8({cur})",
            "sd s3,   5*8({cur})",
            "sd s4,   6*8({cur})",
            "sd s5,   7*8({cur})",
            "sd s6,   8*8({cur})",
            "sd s7,   9*8({cur})",
            "sd s8,  10*8({cur})",
            "sd s9,  11*8({cur})",
            "sd s10, 12*8({cur})",
            "sd s11, 13*8({cur})",
            // Restore next context.
            "ld ra,   0*8({nxt})",
            "ld sp,   1*8({nxt})",
            "ld s0,   2*8({nxt})",
            "ld s1,   3*8({nxt})",
            "ld s2,   4*8({nxt})",
            "ld s3,   5*8({nxt})",
            "ld s4,   6*8({nxt})",
            "ld s5,   7*8({nxt})",
            "ld s6,   8*8({nxt})",
            "ld s7,   9*8({nxt})",
            "ld s8,  10*8({nxt})",
            "ld s9,  11*8({nxt})",
            "ld s10, 12*8({nxt})",
            "ld s11, 13*8({nxt})",
            cur = in(reg) current,
            nxt = in(reg) next,
            // Clobbers: ra and sp are modified, and all callee-saved regs
            // are overwritten.  Caller-saved regs are the caller's problem.
            options(nostack),
        );
    }
}

/// No-op stub so the crate compiles on the host for testing.
#[cfg(not(target_arch = "riscv64"))]
// SAFETY: task stack and entry point are validated during service manifest parsing.
pub unsafe fn context_switch(_current: *mut KernelContext, _next: *const KernelContext) {
    unimplemented!("context_switch is only available on riscv64");
}
