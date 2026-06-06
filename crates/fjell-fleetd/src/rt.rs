#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    fjell_syscall::sys_debug_writeln("fleetd: panic");
    fjell_syscall::sys_exit(1);
}
