//! Bare-metal runtime for this service.
//! The panic handler lives in main.rs to keep the service-specific message.
use core::arch::global_asm;
global_asm!(
    ".section .text.init",
    ".global _start",
    "_start:",
    "  la   sp, __stack_top",
    "  tail service_main",
);
