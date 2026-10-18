//! Bare-metal runtime for fjell-fleetd.
//!
//! RFC-0.37-001 (closes E-078/E-081): this file held only a panic handler —
//! no `_start` symbol, the shape every other service's `rt.rs` provides and
//! `link.ld`'s `ENTRY(_start)` requires. `fjell-fleetd` had never been
//! cross-compiled and linked as a bootable image; this is why, not merely
//! that its own loop never ran. The panic handler moves to `main.rs`,
//! matching every other service in this tree.
use core::arch::global_asm;
global_asm!(
    ".section .text.init",
    ".global _start",
    "_start:",
    "  la   sp, __stack_top",
    "  tail service_main",
);
