#![allow(dead_code)]
//! Hardcoded memory layout for the QEMU `virt` RISC-V machine.
//!
//! Full DTB parsing is deferred to a future milestone.  For M2, these
//! constants are used directly.

/// Physical address at which QEMU loads the kernel image.
pub const RAM_BASE: usize = 0x8000_0000;

/// Per-task device VMA window (RFC 051).
///
/// MMIO mappings are allocated from this reserved range rather than using
/// PA directly as VA (which could collide with user heap / stack).
/// `0x7000_0000..0x8000_0000` = 256 MiB — enough for the PLIC (64 MiB) and
/// other device regions.  Each task's page table is independent, so all tasks
/// can use the same VA range without collision.
pub const DEVICE_VMA_BASE: usize = 0x7000_0000;
pub const DEVICE_VMA_END:  usize = RAM_BASE;  // = 0x8000_0000
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

// ── RFC 035: MmioRegionTable (v0.2.0 — static interim design) ────────────────

/// Lifecycle state of an MMIO region object (RFC 035 §2.2).
///
/// Static table entries are always `Active`.  If a region is revoked (future:
/// via cap-broker lease revocation), it transitions to `Revoked` and all
/// subsequent `sys_mmio_map` calls for that region are rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MmioRegionState {
    /// Region is in use.
    Active,
    /// Region has been revoked; no further mappings are allowed.
    Revoked,
}

/// A single bounded MMIO region (RFC 035 §2.2).
///
/// The static table (interim design) owns these.  In a future milestone the
/// device manager populates the table from the DTB.
#[derive(Clone, Copy, Debug)]
pub struct MmioRegionObject {
    /// Index in the static `MmioRegionTable`.
    pub id:    u32,
    /// Physical base address of this MMIO region.
    pub base:  usize,
    /// Total size in bytes.
    pub size:  usize,
    /// Lifecycle state.
    pub state: MmioRegionState,
    /// Human-readable ASCII description (null-padded to 16 bytes).
    pub description: [u8; 16],
}

impl MmioRegionObject {
    /// True if the region is active and a given `offset + size` is in bounds.
    pub fn is_accessible(&self, offset: usize, size: usize) -> bool {
        self.state == MmioRegionState::Active
            && size > 0
            && offset.saturating_add(size) <= self.size
    }
}

/// Build the static MMIO region table.
///
/// The interim design (RFC 035 §"Static region table") hard-codes the QEMU
/// `virt` device layout.  DTB-driven discovery is deferred to v0.3.
/// RFC 042: region 4 straddles RAM_BASE for the RAM-guard negative test.
pub fn mmio_region_table() -> [MmioRegionObject; MMIO_REGION_COUNT] {
    let make = |id: u32, base: usize, size: usize, desc: &[u8]| {
        let mut d = [0u8; 16];
        for (i, &b) in desc.iter().enumerate().take(15) { d[i] = b; }
        MmioRegionObject {
            id, base, size,
            state: MmioRegionState::Active,
            description: d,
        }
    };
    [
        make(0, 0x0000_0000, 0x1000_0000, b"CLINT/boot-ROM"),
        make(1, 0x1000_0000, 0x0000_1000, b"UART0"),
        make(2, 0x0C00_0000, 0x0400_0000, b"PLIC"),
        make(3, 0x1000_1000, 0x0000_F000, b"virtio-mmio"),
        // RFC 042: synthetic region straddling RAM_BASE (base=0x7FFE_0000, end=0x8001_0000).
        // Mapping offset=0x10000 size=0x20000 crosses RAM_BASE → RAM-guard fires.
        make(4, 0x7FFE_0000, 0x0003_0000, b"neg-test-RAM"),
    ]
}

/// Number of MMIO region entries in the static table.
pub const MMIO_REGION_COUNT: usize = 5;
/// MMIO region index for the virtio-mmio block device region.
pub const MMIO_REGION_VIRTIO: u32 = 3;
