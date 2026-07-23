// ── Bare-metal service runtime ────────────────────────────────────────────────
use core::arch::global_asm;
use core::panic::PanicInfo;
// _start: load stack pointer then jump to service_main.
// GP relaxation is not used (services are small; no GP init needed).
global_asm!(
    ".section .text.init",
    ".global _start",
    "_start:",
    "  la   sp, __stack_top",
    "  tail service_main",
);
#[panic_handler]
fn panic(_: &PanicInfo) -> ! { fjell_syscall::sys_exit(1) }
