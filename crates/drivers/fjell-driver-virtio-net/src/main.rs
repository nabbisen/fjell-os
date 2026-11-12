//! driver-virtio-net — a registration stub. It does not drive the network device.
//!
//! This image is spawned by `init` on every boot, prints one line saying what it is, and
//! exits cleanly (RFC-0.43-003, closes E-118). It holds no capability beyond the ones every
//! image gets, maps no device register, allocates no DMA and binds no interrupt.
//!
//! **It used to claim otherwise.** Until RFC-0.43-003 this file declared six capability slots
//! (`MmioRegion`, two `DmaRegion`s, an `Interrupt`, two `Endpoint`s), a header said `devmgr`
//! installed them, and the console carried *"device descriptor loaded"*, *"features
//! negotiated"* and *"rx queue initialised"*. None of it was true: `devmgr` installs nothing,
//! `sys_mmio_map` was never called, and the first call that needed a capability the image
//! was never given (`sys_irq_bind`) failed and ended the image -- on every boot, behind a
//! green tier, because `TEST:V0.4-NET:PASS` is `netd`'s and never needed this driver.
//!
//! **Why a stub and not a driver.** In this tree services drive their own devices:
//! `storaged` maps its virtio-blk region and allocates its own DMA, and
//! `driver-virtio-blk` is the same shape as this file. Making this image a real driver would
//! mean granting a driver image DMA, an interrupt and endpoints -- a privileged driver plane,
//! which is an authority design (which plane owns DMA, how an interrupt reaches a user task,
//! whether three images should share the whole virtio-mmio range) that is neither decided nor
//! foreclosed here. This image stops *claiming* to be that driver; a working NIC is a later
//! line that earns its grants.
//!
//! The host-testable virtqueue, ring, state-machine and feature-negotiation code in
//! `src/core/` (the `fjell_driver_virtio_net` library) is untouched and is **not linked into
//! this image**.
#![no_std]
#![no_main]
mod rt;

use fjell_syscall::{sys_debug_writeln, sys_exit};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    sys_debug_writeln("driver-virtio-net: panic");
    sys_exit(1);
}

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    // One line, and it says what this is. It must not read as device work done.
    sys_debug_writeln("driver-virtio-net: registration stub, not wired to the device");
    sys_exit(0)
}
