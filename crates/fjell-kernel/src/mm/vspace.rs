//! Address space abstraction: `VmPerms`, `VmRegion`, `AddressSpace`.
#![allow(dead_code)]

use super::{
    address::{PhysFrame, VirtAddr},
    error::MmError,
    frame_alloc::FrameAllocator,
    page_table,
    region::VmRegionKind,
};

// ── VmPerms ───────────────────────────────────────────────────────────────────

/// Page permission flags (mirrors Sv39 PTE R/W/X/U bits).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct VmPerms(u8);

impl VmPerms {
    pub const R: VmPerms = VmPerms(0b0001); // Readable
    pub const W: VmPerms = VmPerms(0b0010); // Writable
    pub const X: VmPerms = VmPerms(0b0100); // Executable
    pub const U: VmPerms = VmPerms(0b1000); // User-accessible

    pub fn empty() -> Self { VmPerms(0) }

    pub fn contains(self, other: VmPerms) -> bool {
        self.0 & other.0 == other.0
    }
}

impl core::ops::BitOr for VmPerms {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self { VmPerms(self.0 | rhs.0) }
}

impl core::ops::BitOrAssign for VmPerms {
    fn bitor_assign(&mut self, rhs: Self) { self.0 |= rhs.0; }
}

// ── VmRegion ──────────────────────────────────────────────────────────────────

/// A contiguous virtual memory region within an address space.
#[derive(Clone, Copy, Debug)]
pub struct VmRegion {
    pub start: VirtAddr,
    pub end:   VirtAddr,   // exclusive
    pub perms: VmPerms,
    pub kind:  VmRegionKind,
}

// ── AddressSpace ──────────────────────────────────────────────────────────────

/// Maximum number of `VmRegion` entries per address space.
pub const MAX_VM_REGIONS: usize = 16;

/// Unique identifier for an address space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AddressSpaceId(pub u16);

/// A Sv39 virtual address space.
///
/// Owns the root page-table frame.  Region metadata is tracked inline
/// without heap allocation.
pub struct AddressSpace {
    pub id:       AddressSpaceId,
    pub root:     PhysFrame,
    pub asid:     u16,          // 0 for M2 (ASID allocator deferred)
    regions_len:  usize,
    regions:      [Option<VmRegion>; MAX_VM_REGIONS],
}

impl AddressSpace {
    /// Create a new, empty address space backed by `root_frame`.
    pub fn new(id: AddressSpaceId, root_frame: PhysFrame) -> Self {
        // Zero the root page table.
        // SAFETY: root_frame is freshly allocated and 4-KiB aligned.
        unsafe {
            core::ptr::write_bytes(root_frame.pa() as *mut u8, 0, 4096);
        }
        AddressSpace {
            id,
            root: root_frame,
            asid: 0,
            regions_len: 0,
            regions: [None; MAX_VM_REGIONS],
        }
    }

    /// Map a single page into this address space.
    ///
    /// Also registers a `VmRegion` covering the page (coalescing is not
    /// performed — each `map_page` call adds one region entry).
    pub fn map_page(
        &mut self,
        va: VirtAddr,
        frame: PhysFrame,
        perms: VmPerms,
        kind: VmRegionKind,
        fa: &mut FrameAllocator<'_>,
    ) -> Result<(), MmError> {
        // Invariant MM-VM-001: kernel pages must not be mapped U=1.
        debug_assert!(
            !(kind == VmRegionKind::KernelShared && perms.contains(VmPerms::U)),
            "kernel pages must not have U=1"
        );
        // Invariant MM-VM-003: user text must not have W.
        debug_assert!(
            !(kind == VmRegionKind::UserText && perms.contains(VmPerms::W)),
            "user text must not have W"
        );

        // SAFETY: root.pa() is a valid Sv39 root page table.
        // sfence.vma is called by the caller after all maps are done.
        unsafe {
            page_table::map_page(self.root.pa(), va, frame, perms, fa)?;
        }

        if self.regions_len < MAX_VM_REGIONS {
            self.regions[self.regions_len] = Some(VmRegion {
                start: va,
                end: VirtAddr(va.0 + 4096),
                perms,
                kind,
            });
            self.regions_len += 1;
        }
        Ok(())
    }

    /// Unmap a single page, returning its physical frame.
    pub fn unmap_page(&mut self, va: VirtAddr) -> Result<PhysFrame, MmError> {
        // SAFETY: root.pa() is a valid Sv39 root page table.
        let frame = unsafe { page_table::unmap_page(self.root.pa(), va)? };

        // Remove the region entry.
        for slot in self.regions.iter_mut() {
            if let Some(r) = slot {
                if r.start == va {
                    *slot = None;
                    self.regions_len -= 1;
                    break;
                }
            }
        }
        Ok(frame)
    }

    /// Translate a virtual address to (frame, permissions).
    pub fn translate(&self, va: VirtAddr) -> Result<(PhysFrame, VmPerms), MmError> {
        // SAFETY: root.pa() is a valid Sv39 root page table.
        unsafe { page_table::translate(self.root.pa(), va) }
    }

    /// Copy the kernel-half entries (VPN[2] >= 256) from `kernel_root`.
    ///
    /// Invariant MM-VM-002: all address spaces share the same kernel half.
    pub fn clone_kernel_half(&mut self, kernel_root: PhysFrame) {
        // SAFETY: both root page tables are valid and 4-KiB aligned.
        unsafe {
            page_table::clone_kernel_half(self.root.pa(), kernel_root.pa());
        }
    }

    /// Physical frame number of the root page table (for `satp`).
    #[inline]
    pub fn root_pfn(&self) -> usize {
        self.root.pfn as usize
    }
}
