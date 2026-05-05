//! Boot entry for Fjell OS on RISC-V 64.
//!
//! Contains the `_start` symbol placed in `.text.init` so the linker script
//! guarantees it is the very first instruction in the kernel image.
//!
//! Responsibilities (M1):
//!   1. Read `mhartid`; park all harts except hart 0.
//!   2. Zero the BSS section.
//!   3. Set the stack pointer to `__stack_top`.
//!   4. Jump to `kmain`.
//!
//! In M2 this shim will be extended to configure M-mode trap delegation and
//! transfer control to S-mode before calling `kmain(hart_id, dtb_pa)`.
//!
//! # cfg guard
//! The `global_asm!` block is gated on `target_arch = "riscv64"` so that
//! `cargo build --workspace` (which targets the host) can compile this crate
//! without encountering unknown RISC-V instructions.  The kernel binary is
//! only meaningful when cross-compiled for `riscv64gc-unknown-none-elf`.

#[cfg(target_arch = "riscv64")]
use core::arch::global_asm;

#[cfg(target_arch = "riscv64")]
global_asm!(
    r#"
    .section .text.init
    .global _start
_start:
    # ── 1. Hart selection ────────────────────────────────────────────────
    # Only hart 0 continues; all others spin in `park`.
    csrr    t0, mhartid
    bnez    t0, park

    # ── 2. BSS zero-fill ─────────────────────────────────────────────────
    la      a0, __bss_start
    la      a1, __bss_end
    bgeu    a0, a1, 2f      # skip if BSS is empty
1:
    sd      zero, (a0)
    addi    a0, a0, 8
    bltu    a0, a1, 1b
2:

    # ── 3. Stack pointer ──────────────────────────────────────────────────
    la      sp, __stack_top

    # ── 4. Enter Rust kernel main ─────────────────────────────────────────
    call    kmain

    # kmain must never return; halt here as a safety net.
halt:
    wfi
    j       halt

    # ── Park loop for non-boot harts ──────────────────────────────────────
park:
    wfi
    j       park
    "#
);
