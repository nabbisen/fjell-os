#![no_std]
#![no_main]
mod rt;
use fjell_proxy_text as _;
use fjell_syscall::{sys_debug_writeln, sys_exit};
#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("M5: proxy-text started");
    sys_exit(0)
}
