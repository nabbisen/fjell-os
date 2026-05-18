use core::arch::global_asm;
use core::panic::PanicInfo;
global_asm!(".section .text.init",".global _start","_start:","  la sp, __stack_top","  tail service_main",);
#[panic_handler]
fn panic(_: &PanicInfo) -> ! { fjell_syscall::sys_exit(1) }
