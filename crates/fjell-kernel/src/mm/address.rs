//! Physical and virtual address wrapper types.
#![allow(dead_code)]

use super::error::MmError;

/// A physical memory address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PhysAddr(pub usize);

impl PhysAddr {
    /// Convert to a physical page-frame number.
    #[inline]
    pub fn pfn(self) -> u64 {
        (self.0 >> 12) as u64
    }

    /// Align up to the nearest page boundary.
    #[inline]
    pub fn align_up(self) -> Self {
        PhysAddr((self.0 + 0xFFF) & !0xFFF)
    }

    /// Align down to the nearest page boundary.
    #[inline]
    pub fn align_down(self) -> Self {
        PhysAddr(self.0 & !0xFFF)
    }
}

/// A virtual memory address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct VirtAddr(pub usize);

impl VirtAddr {
    /// Return `true` if this address falls in the canonical user range
    /// (bits [63:39] are all zero).
    #[inline]
    pub fn is_user(self) -> bool {
        self.0 >> 39 == 0
    }

    /// Return `true` if this address is in the canonical kernel half
    /// (bits [63:39] are all one for Sv39).
    #[inline]
    pub fn is_kernel(self) -> bool {
        (self.0 >> 39) == (usize::MAX >> 39)
    }
}

/// A physical page frame (4 KiB aligned, identified by its PFN).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysFrame {
    pub pfn: u64,
}

impl PhysFrame {
    /// Create a frame from a physical address, checking alignment.
    pub fn from_pa(pa: usize) -> Result<Self, MmError> {
        if pa & 0xFFF != 0 {
            return Err(MmError::Misaligned);
        }
        Ok(PhysFrame { pfn: (pa >> 12) as u64 })
    }

    /// Physical address of the start of this frame.
    #[inline]
    pub fn pa(self) -> usize {
        (self.pfn as usize) << 12
    }
}
