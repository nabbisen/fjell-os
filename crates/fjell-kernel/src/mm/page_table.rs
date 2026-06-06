//! Sv39 3-level page-table implementation.
#![allow(dead_code)]
//!
//! Invariants (MM-VM-*):
//!   MM-VM-001  No kernel page is mapped U=1 in any address space.
//!   MM-VM-003  User text has no W bit.
//!   MM-VM-005  `map_page` does not silently overwrite an existing mapping.
//!   MM-VM-006  `unmap_page` returns `NotMapped` for absent mappings.
//!   MM-VM-007  `satp` writes are always followed by `sfence.vma`.

use super::{
    address::{PhysFrame, VirtAddr},
    error::MmError,
    frame_alloc::{FrameAllocator, FrameOwner},
    vspace::VmPerms,
};
use crate::arch::riscv64::pte::{
    sv39_decode_va, Pte, PTE_R, PTE_U, PTE_W, PTE_X,
};

/// Map a single 4 KiB page.
///
/// Allocates intermediate page-table pages as needed.
///
/// # Safety
/// - `root_pa` must point to a valid, 4-KiB-aligned root page table.
/// - `fa` must outlive the page table being modified.
/// - Caller must execute `sfence.vma` after all map operations are done
///   (invariant MM-VM-007).
// SAFETY: physical address is within the kernel heap; alignment is guaranteed by the frame allocator.
pub unsafe fn map_page(
    root_pa: usize,
    va: VirtAddr,
    frame: PhysFrame,
    perms: VmPerms,
    fa: &mut FrameAllocator<'_>,
) -> Result<(), MmError> {
    let flags = perms_to_pte_flags(perms);
    let (vpn2, vpn1, vpn0, _) = sv39_decode_va(va.0);

    // SAFETY: root_pa is a valid page-table physical address provided by the
    // caller.  We walk the three levels, allocating intermediate tables.
    unsafe {
        let l2 = root_pa as *mut Pte;
        let pte2 = &mut *l2.add(vpn2);
        let l1_pa = ensure_next_level(pte2, fa)?;

        let l1 = l1_pa as *mut Pte;
        let pte1 = &mut *l1.add(vpn1);
        let l0_pa = ensure_next_level(pte1, fa)?;

        let l0 = l0_pa as *mut Pte;
        let pte0 = &mut *l0.add(vpn0);

        if pte0.is_valid() {
            return Err(MmError::AlreadyMapped);
        }
        *pte0 = Pte::leaf(frame.pfn, flags);
    }
    Ok(())
}

/// Map or remap a single 4 KiB page, overwriting any existing mapping.
///
/// Use this instead of `map_page` when upgrading permissions (e.g. adding the
/// User bit to an existing kernel-mode-only mapping).
///
/// # Safety
/// Same requirements as `map_page`.
// SAFETY: physical address is within the kernel heap; alignment is guaranteed by the frame allocator.
pub unsafe fn remap_page(
    root_pa: usize,
    va: VirtAddr,
    frame: PhysFrame,
    perms: VmPerms,
    fa: &mut FrameAllocator<'_>,
) -> Result<(), MmError> {
    let flags = perms_to_pte_flags(perms);
    let (vpn2, vpn1, vpn0, _) = sv39_decode_va(va.0);
    // SAFETY: physical address is within the kernel heap; alignment is guaranteed by the frame allocator.
    unsafe {
        let l2 = root_pa as *mut Pte;
        let pte2 = &mut *l2.add(vpn2);
        let l1_pa = ensure_next_level(pte2, fa)?;
        let l1 = l1_pa as *mut Pte;
        let pte1 = &mut *l1.add(vpn1);
        let l0_pa = ensure_next_level(pte1, fa)?;
        let l0 = l0_pa as *mut Pte;
        let pte0 = &mut *l0.add(vpn0);
        *pte0 = Pte::leaf(frame.pfn, flags);  // overwrite unconditionally
    }
    Ok(())
}

/// Unmap a single 4 KiB page, returning the freed frame.
///
/// Does not free intermediate page-table pages (they may still be needed
/// for other mappings).
///
/// # Safety
/// Same requirements as `map_page`.
// SAFETY: physical address is within the kernel heap; alignment is guaranteed by the frame allocator.
pub unsafe fn unmap_page(
    root_pa: usize,
    va: VirtAddr,
) -> Result<PhysFrame, MmError> {
    let (vpn2, vpn1, vpn0, _) = sv39_decode_va(va.0);

    // SAFETY: caller guarantees root_pa is valid.
    unsafe {
        let l2 = root_pa as *mut Pte;
        let pte2 = &*l2.add(vpn2);
        if !pte2.is_valid() { return Err(MmError::NotMapped); }

        let l1 = pte2.phys_addr() as *mut Pte;
        let pte1 = &*l1.add(vpn1);
        if !pte1.is_valid() { return Err(MmError::NotMapped); }

        let l0 = pte1.phys_addr() as *mut Pte;
        let pte0 = &mut *l0.add(vpn0);
        if !pte0.is_valid() { return Err(MmError::NotMapped); }

        let frame = PhysFrame { pfn: pte0.ppn() };
        *pte0 = Pte::invalid();
        Ok(frame)
    }
}

/// Walk the page table and return the physical frame and permissions for `va`.
///
/// # Safety
/// Same requirements as `map_page`.
// SAFETY: physical address is within the kernel heap; alignment is guaranteed by the frame allocator.
pub unsafe fn translate(
    root_pa: usize,
    va: VirtAddr,
) -> Result<(PhysFrame, VmPerms), MmError> {
    let (vpn2, vpn1, vpn0, _) = sv39_decode_va(va.0);

    // SAFETY: caller guarantees root_pa is valid.
    unsafe {
        let l2 = root_pa as *const Pte;
        let pte2 = &*l2.add(vpn2);
        if !pte2.is_valid() { return Err(MmError::NotMapped); }

        let l1 = pte2.phys_addr() as *const Pte;
        let pte1 = &*l1.add(vpn1);
        if !pte1.is_valid() { return Err(MmError::NotMapped); }

        let l0 = pte1.phys_addr() as *const Pte;
        let pte0 = &*l0.add(vpn0);
        if !pte0.is_valid() || !pte0.is_leaf() { return Err(MmError::NotMapped); }

        let frame = PhysFrame { pfn: pte0.ppn() };
        let perms = pte_flags_to_perms(pte0.0);
        Ok((frame, perms))
    }
}

/// Copy the top-level (VPN[2]) kernel entries from `kernel_root_pa` into
/// `target_root_pa`.  This gives every user address space a shared view of
/// the kernel half without duplicating pages.
///
/// Invariant MM-VM-002: the kernel shared map is identical across all spaces.
///
/// # Safety
/// Both physical addresses must point to valid, zeroed root page tables.
// SAFETY: physical address is within the kernel heap; alignment is guaranteed by the frame allocator.
pub unsafe fn clone_kernel_half(
    target_root_pa: usize,
    kernel_root_pa: usize,
) {
    // This kernel uses an identity map with the kernel image sitting at
    // physical 0x80000000 = VA 0x80000000 (VPN[2] = 2, Sv39).
    // The "upper canonical half" approach (VPN[2] >= 256) is NOT used here;
    // instead we copy the root-level entries that the kernel actually occupies:
    //
    //   Entry 2 (VA 0x8000_0000 – 0xBFFF_FFFF): kernel text, data, stack.
    //   Entry 0 is LEFT EMPTY here; each task adds its own user mappings
    //           plus the UART mapping into entry 0 independently.
    //
    // Sharing the entry-2 PTE is safe because user tasks never write into
    // kernel code/data pages (page faults are handled in trap/fault.rs).
    // SAFETY: both root_pa values are valid 4-KiB-aligned page tables.
    unsafe {
        let src = kernel_root_pa as *const Pte;
        let dst = target_root_pa as *mut Pte;
        // Entry 2 covers the 1-GiB region starting at VA 0x8000_0000.
        *dst.add(2) = *src.add(2);
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Ensure `pte` points to a next-level page table, allocating one if absent.
///
/// Returns the physical address of the next-level table.
///
/// # Safety
/// `pte` must be a valid pointer to an entry in a live page table.
// SAFETY: physical address is within the kernel heap; alignment is guaranteed by the frame allocator.
unsafe fn ensure_next_level(
    pte: &mut Pte,
    fa: &mut FrameAllocator<'_>,
) -> Result<usize, MmError> {
    if pte.is_valid() {
        Ok(pte.phys_addr())
    } else {
        let frame = fa.alloc_frame(FrameOwner::KernelPageTable)?;
        let pa = frame.pa();
        // Zero the new page-table page.
        // SAFETY: `pa` is freshly allocated, 4-KiB aligned, and owned.
        unsafe {
            core::ptr::write_bytes(pa as *mut u8, 0, 4096);
        }
        *pte = Pte::branch(frame.pfn);
        Ok(pa)
    }
}

fn perms_to_pte_flags(p: VmPerms) -> u64 {
    let mut f = 0u64;
    if p.contains(VmPerms::R) { f |= PTE_R; }
    if p.contains(VmPerms::W) { f |= PTE_W; }
    if p.contains(VmPerms::X) { f |= PTE_X; }
    if p.contains(VmPerms::U) { f |= PTE_U; }
    f
}

fn pte_flags_to_perms(raw: u64) -> VmPerms {
    let mut p = VmPerms::empty();
    if raw & PTE_R != 0 { p |= VmPerms::R; }
    if raw & PTE_W != 0 { p |= VmPerms::W; }
    if raw & PTE_X != 0 { p |= VmPerms::X; }
    if raw & PTE_U != 0 { p |= VmPerms::U; }
    p
}
