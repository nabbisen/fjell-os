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

    # ── 1b. Translation off, and what a reset leaves behind ──────────────
    # A cold boot starts with satp = 0, so this was never needed until the
    # machine could reset itself (RFC-0.33-001 D15). QEMU's system reset leaves
    # satp as the previous boot set it, so the second boot's first page-table
    # write faulted with the previous kernel's mapping still live, and the trap
    # handler then faulted on its own first store: a silent storm, six lines of
    # output and nothing more. Found by running the reset WITHOUT -no-reboot,
    # which the harness's reset tier cannot do (it passes -no-reboot); the
    # `reboot-again` tier now does, and counts the boots.
    #
    # satp is the only supervisor CSR that NEEDS clearing. Why the rest do not
    # (RFC-0.33-001 D20 -- read out of the tree, not assumed):
    #   * mstatus  m_mode_setup writes it WHOLESALE (`1 << 11`, MPP = S), which
    #              clears MIE and MPIE: mret leaves S-mode interrupts disabled,
    #              so nothing stale can be reached through an interrupt before
    #              the kernel installs its own handler.
    #   * PMP      pmpaddr0/pmpcfg0 are rewritten on every boot.
    #   * sie, stvec, sscratch, sepc, scause
    #              SURVIVE a reset and are harmless ONLY while no exception
    #              occurs between mret and the kernel's own stvec write. That
    #              holds because satp is zero here (no translation, so no page
    #              fault) and PMP is permissive. Whoever adds an early store,
    #              load or instruction that can trap before that write is
    #              breaking this, and must install stvec first or clear these.
    #   * stimecmp only if SSTC is ever used; it is not.
    # Clearing registers blindly is not better than knowing why they need no
    # clearing: this is the reasoning, not a list.
    csrw    satp, zero
    sfence.vma

    # ── 2. BSS zero-fill ─────────────────────────────────────────────────
    # Uses t1/t2, NOT a0/a1: a1 is the device-tree pointer firmware passes
    # (RFC-0.33-001 D22, E-064). This loop used to load __bss_end into a1, so
    # the kernel received dtb_pa = __bss_end (0x8007ccb8 in that build) three
    # lines above a comment saying it did not touch a1.
    la      t1, __bss_start
    la      t2, __bss_end
    bgeu    t1, t2, 2f
1:
    sd      zero, (t1)
    addi    t1, t1, 8
    bltu    t1, t2, 1b
2:

    # ── 3. Stack pointer ──────────────────────────────────────────────────
    la      sp, __stack_top

    # a0 = hart id (reloaded: it is the CSR's, not a firmware value we hold),
    # a1 = the firmware's dtb_pa, untouched since entry.
    csrr    a0, mhartid

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
