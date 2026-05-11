//! Boot entry for Fjell OS on RISC-V 64.
//!
//! M2 adds an M-mode shim that:
//!   1. Selects hart 0 (parks others).
//!   2. Zeros BSS.
//!   3. Sets the early stack pointer.
//!   4. Configures trap delegation (all exceptions + timer/software interrupts
//!      delegated to S-mode).
//!   5. Transfers control to S-mode via `mret`, calling `s_mode_entry`.
//!
//! S-mode receives `hart_id` (a0) and `dtb_pa` (a1) forwarded from firmware.

#[cfg(target_arch = "riscv64")]
use core::arch::global_asm;

#[cfg(target_arch = "riscv64")]
global_asm!(
    r#"
    .section .text.init
    .global _start
_start:
    # ── 1. Hart selection ────────────────────────────────────────────────
    csrr    t0, mhartid
    bnez    t0, park

    # ── 2. BSS zero-fill ─────────────────────────────────────────────────
    la      a0, __bss_start
    la      a1, __bss_end
    bgeu    a0, a1, 2f
1:
    sd      zero, (a0)
    addi    a0, a0, 8
    bltu    a0, a1, 1b
2:

    # ── 3. Stack pointer ──────────────────────────────────────────────────
    la      sp, __stack_top

    # Preserve hart_id (a0) and dtb_pa (a1) that firmware placed in registers.
    # Reload them after BSS clear overwrote a0/a1 above.
    csrr    a0, mhartid
    # dtb_pa is in a1 from firmware — we do not touch it.

    # ── 4. M-mode shim ───────────────────────────────────────────────────
    call    m_mode_setup

    # m_mode_setup calls mret and never returns.
halt:
    wfi
    j       halt

park:
    wfi
    j       park
    "#
);
