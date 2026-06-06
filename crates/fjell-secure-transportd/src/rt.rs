//! Bare-metal runtime for fjell-secure-transportd.
use core::arch::global_asm;
global_asm!(
    ".section .text.init",
    ".global _start",
    "_start:",
    "  la   sp, __stack_top",
    "  tail service_main",
);
