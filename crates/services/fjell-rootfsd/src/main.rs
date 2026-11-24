#![no_std]
#![no_main]
mod rt;
use fjell_syscall::sys_exit;
// SAFETY: category=symbol-export `service_main` is this service's entry symbol, tail-called by `_start` in rt.rs; no other item in the image exports the name.
#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_exit(0)
}
