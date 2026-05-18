#![no_std]
#![no_main]
mod rt;
use fjell_syscall::sys_exit;
#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! { sys_exit(0) }
