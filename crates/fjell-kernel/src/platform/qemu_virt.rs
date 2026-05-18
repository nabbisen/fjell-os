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

// ── RFC 016: MmioRegionTable ──────────────────────────────────────────────────

/// A single bounded MMIO region that a driver may request via `sys_mmio_map`.
#[derive(Clone, Copy, Debug)]
pub struct MmioRegionObject {
    /// Physical base address of this MMIO region.
    pub base: usize,
    /// Total size in bytes.
    pub size: usize,
    /// Human-readable ASCII description (null-padded to 16 bytes).
    pub description: [u8; 16],
}

/// Build the static MMIO region table from `MMIO_REGIONS`.
///
/// Called once at kernel init to populate `MmioRegionObject` entries that
/// are then installed as `MmioRegion` capabilities in init's CSpace.
pub fn mmio_region_table() -> [MmioRegionObject; 4] {
    let make = |base: usize, size: usize, desc: &[u8]| {
        let mut d = [0u8; 16];
        for (i, &b) in desc.iter().enumerate().take(15) { d[i] = b; }
        MmioRegionObject { base, size, description: d }
    };
    [
        make(0x0000_0000, 0x1000_0000, b"CLINT/boot-ROM"),
        make(0x1000_0000, 0x0000_1000, b"UART0"),
        make(0x0C00_0000, 0x0400_0000, b"PLIC"),
        make(0x1000_1000, 0x0000_F000, b"virtio-mmio"),
    ]
}

/// Number of MMIO region entries in the static table.
pub const MMIO_REGION_COUNT: usize = 4;
/// MMIO region index for the virtio-mmio block device region.
pub const MMIO_REGION_VIRTIO: u32 = 3;
