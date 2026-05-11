#![allow(dead_code)]
//! Hardcoded memory layout for the QEMU `virt` RISC-V machine.
//!
//! Full DTB parsing is deferred to a future milestone.  For M2, these
//! constants are used directly.

/// Physical address at which QEMU loads the kernel image.
pub const RAM_BASE: usize = 0x8000_0000;
/// Default RAM size in the QEMU `virt` machine (128 MiB).
pub const RAM_SIZE: usize = 128 * 1024 * 1024;
/// Exclusive end of RAM.
pub const RAM_END: usize = RAM_BASE + RAM_SIZE;

/// MMIO regions that must be reserved in the frame allocator.
pub const MMIO_REGIONS: &[(usize, usize, &str)] = &[
    (0x0000_0000, 0x1000_0000, "CLINT/boot-ROM/test"),
    (0x1000_0000, 0x1000_1000, "UART0"),
    (0x0C00_0000, 0x1000_0000, "PLIC"),
    (0x1000_1000, 0x1001_0000, "virtio"),
];

/// Summary of platform memory layout passed to kernel init.
pub struct PlatformInfo {
    /// Physical address of the start of RAM.
    pub ram_base: usize,
    /// Total RAM size in bytes.
    pub ram_size: usize,
    /// DTB physical address forwarded from M-mode (0 if unavailable).
    pub dtb_pa: usize,
}

/// Return the hardcoded QEMU `virt` platform layout.
pub fn qemu_virt_platform() -> PlatformInfo {
    PlatformInfo {
        ram_base: RAM_BASE,
        ram_size: RAM_SIZE,
        dtb_pa: 0,
    }
}
