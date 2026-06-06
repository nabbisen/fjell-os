//! `satp` register helpers for Sv39 virtual memory.
#![allow(dead_code)]

/// Sv39 mode value written to `satp[63:60]`.
pub const SATP_MODE_SV39: usize = 8 << 60;

/// Write `satp` and execute `sfence.vma` to flush the TLB.
///
/// `root_pfn` is the physical page-frame number of the root page table.
/// ASID is fixed at 0 for M2.
///
/// # Safety
/// - `root_pfn` must point to a correctly-constructed Sv39 root page table.
/// - All kernel mappings required for the current execution context must be
///   present in that page table before calling this function.
/// - Caller must ensure no concurrent modification of any page table while
///   this function executes.
/// - The `sfence.vma` fence that follows the `satp` write is mandatory to
///   invalidate stale TLB entries (invariant MM-VM-007).
#[inline]
// SAFETY: called only during kernel init before MMU is enabled; register access is M/S-mode only.
pub unsafe fn enable_sv39(root_pfn: usize) {
    let satp_val = SATP_MODE_SV39 | (root_pfn & 0x0FFF_FFFF_FFFF);
    // SAFETY: caller guarantees the page table is valid and the kernel
    // continues to be reachable after the address translation switch.
    unsafe {
        core::arch::asm!(
            "csrw satp, {satp}",
            "sfence.vma zero, zero",
            satp = in(reg) satp_val,
        );
    }
}

/// Execute a global `sfence.vma` (all addresses, all ASIDs).
///
/// Must be called after any page-table modification before the next memory
/// access that depends on the updated mapping.
///
/// # Safety
/// Caller must ensure this is called in a context where a TLB flush is safe
/// (i.e. no concurrent hart is relying on the stale mapping being valid).
#[inline]
// SAFETY: called only during kernel init before MMU is enabled; register access is M/S-mode only.
pub unsafe fn sfence_vma_all() {
    // SAFETY: caller upholds the synchronisation requirement.
    unsafe { core::arch::asm!("sfence.vma zero, zero") };
}
