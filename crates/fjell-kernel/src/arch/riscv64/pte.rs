//! Sv39 Page Table Entry (PTE) and page-table constants.
//! Items unused in M2 are kept for M3+ and are intentionally suppressed.
#![allow(dead_code)]

/// Size of one page (4 KiB).
pub const PAGE_SIZE: usize = 4096;
/// Number of PTE entries per page table (512 × 8 bytes = 4 KiB).
pub const PTE_PER_PAGE: usize = 512;
/// Number of page-table levels in Sv39.
pub const SV39_LEVELS: usize = 3;

// PTE flag bits.
pub const PTE_V: u64 = 1 << 0; // Valid
pub const PTE_R: u64 = 1 << 1; // Readable
pub const PTE_W: u64 = 1 << 2; // Writable
pub const PTE_X: u64 = 1 << 3; // Executable
pub const PTE_U: u64 = 1 << 4; // User-accessible
pub const PTE_G: u64 = 1 << 5; // Global mapping
pub const PTE_A: u64 = 1 << 6; // Accessed (hardware-managed)
pub const PTE_D: u64 = 1 << 7; // Dirty    (hardware-managed)

/// A single Sv39 page-table entry stored as a `u64`.
#[derive(Clone, Copy, Default)]
#[repr(transparent)]
pub struct Pte(pub u64);

impl Pte {
    /// Construct an invalid (zero) PTE.
    #[inline]
    pub const fn invalid() -> Self {
        Pte(0)
    }

    /// Construct a leaf PTE from a physical page-frame number and flags.
    ///
    /// The PPN occupies bits [53:10] in the 64-bit PTE word.
    #[inline]
    pub fn leaf(ppn: u64, flags: u64) -> Self {
        Pte(((ppn & 0x0FFF_FFFF_FFFF) << 10) | flags | PTE_V | PTE_A | PTE_D)
    }

    /// Construct a non-leaf (pointer) PTE that points to the next-level table.
    #[inline]
    pub fn branch(ppn: u64) -> Self {
        Pte(((ppn & 0x0FFF_FFFF_FFFF) << 10) | PTE_V)
    }

    /// Is the valid bit set?
    #[inline]
    pub fn is_valid(self) -> bool {
        self.0 & PTE_V != 0
    }

    /// Is this a leaf PTE (has R, W, or X set)?
    #[inline]
    pub fn is_leaf(self) -> bool {
        self.0 & (PTE_R | PTE_W | PTE_X) != 0
    }

    /// Extract the physical page-frame number.
    #[inline]
    pub fn ppn(self) -> u64 {
        (self.0 >> 10) & 0x0FFF_FFFF_FFFF
    }

    /// Extract the physical address this PTE points to.
    #[inline]
    pub fn phys_addr(self) -> usize {
        (self.ppn() as usize) << 12
    }
}

/// Decode a Sv39 virtual address into its three VPN components and page offset.
///
/// Returns `(vpn[2], vpn[1], vpn[0], offset)`.
#[inline]
pub fn sv39_decode_va(va: usize) -> (usize, usize, usize, usize) {
    let vpn2 = (va >> 30) & 0x1FF;
    let vpn1 = (va >> 21) & 0x1FF;
    let vpn0 = (va >> 12) & 0x1FF;
    let offset = va & 0xFFF;
    (vpn2, vpn1, vpn0, offset)
}
