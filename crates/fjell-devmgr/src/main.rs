//! Device Manager — M6.
//! Discovers virtio-mmio block device from platform info.
#![no_std]
#![no_main]
mod rt;
use fjell_syscall::{sys_exit, sys_debug_writeln, sys_platform_info_get};

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("M6: devmgr started");
    // Query kernel for platform device list.
    match sys_platform_info_get() {
        Ok(base) if base == 0x1000_1000 => {
            sys_debug_writeln("M6: virtio-mmio blk discovered");
        }
        _ => {
            sys_debug_writeln("M6: devmgr: no virtio-blk found");
        }
    }
    sys_exit(0)
}
