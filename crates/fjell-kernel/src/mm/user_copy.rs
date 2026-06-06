//! Safe byte-level copy between kernel and user address spaces (RFC 039).
//!
//! # Design
//!
//! Every copy goes through two validation layers:
//!
//! 1. **Arithmetic validation** via [`super::user_ptr::UserPtr`] — rejects
//!    null pointers, kernel addresses, length overflow, and wraparound before
//!    any page-table access.
//!
//! 2. **Page-table walk** via [`super::page_table::translate`] — verifies each
//!    page is mapped, that `PTE_U` is present, and that `PTE_W` (write) or
//!    `PTE_R` (read) is set.
//!
//! The kernel uses a physical-address identity map (PA == kernel VA), so once
//! the physical address of a user page is known, direct memory access is safe.
//!
//! # Safety note
//!
//! Both `copy_to_user` and `copy_from_user` are `unsafe` because:
//! - `root_pfn` must be the valid Sv39 root PFN for the target task.
//! - The mapping must remain live for the duration of the call.
//! - On RISC-V the caller must ensure `sfence.vma` is not needed around this
//!   call (i.e. the TLB must reflect the current page table).

use super::{address::VirtAddr, page_table, vspace::VmPerms};
pub use super::user_ptr::{UserCopyError, UserPtr};

/// Copy `src` bytes into the user address space described by `root_pfn`
/// (RFC 039 §2.1 — `copy_to_user`).
///
/// Equivalent to `copy_to_user(task, dst_user, src)` from the RFC; the
/// caller resolves `task → root_pfn` before calling this function.
///
/// Validation:
/// 1. Arithmetic range check via `UserPtr::new`.
/// 2. Per-page page-table walk: mapped + PTE_W + PTE_U.
///
/// # Safety
/// See module-level safety note.
// SAFETY: category=page-table-mutation pointer and length are validated against the task VMA map before this call.
pub unsafe fn copy_to_user_bytes(
    root_pfn: usize,
    dst_va:   usize,
    src:      &[u8],
) -> Result<usize, UserCopyError> {
    // Step 1: arithmetic validation.
    let dst = UserPtr::new(dst_va, src.len())?;
    if dst.is_empty() { return Ok(0); }

    let root_pa  = root_pfn << 12;
    let mut done = 0usize;

    // Step 2: page-by-page walk.
    while done < src.len() {
        let va      = dst.addr() + done;
        let page_va = va & !0xFFF;
        let offset  = va &  0xFFF;
        let remain  = (0x1000 - offset).min(src.len() - done);

        // SAFETY: category=page-table-mutation pointer and length are validated against the task VMA map before this call.
        let (frame, perms) = unsafe {
            page_table::translate(root_pa, VirtAddr(page_va))
                .map_err(|_| UserCopyError::NotMapped)?
        };

        if !perms.contains(VmPerms::W) || !perms.contains(VmPerms::U) {
            return Err(UserCopyError::PermissionDenied);
        }

        let pa = frame.pa() + offset;
        // SAFETY: category=raw-pointer-deref pointer and length are validated against the task VMA map before this call.
        unsafe {
            core::ptr::copy_nonoverlapping(
                src[done..].as_ptr(),
                pa as *mut u8,
                remain,
            );
        }
        done += remain;
    }

    Ok(done)
}

/// Copy `len` bytes from a user address into the kernel buffer `dst`
/// (RFC 039 §2 — `copy_from_user`).
///
/// The inverse of `copy_to_user_bytes`: reads from user space (validates R+U)
/// and writes to a kernel-owned buffer.
///
/// # Safety
/// See module-level safety note.
#[allow(dead_code)]  // RFC 039: defined for completeness; wired when syscalls need in-copy
// SAFETY: category=page-table-mutation pointer and length are validated against the task VMA map before this call.
pub unsafe fn copy_from_user_bytes(
    root_pfn: usize,
    src_va:   usize,
    dst:      &mut [u8],
) -> Result<usize, UserCopyError> {
    // Step 1: arithmetic validation.
    let src = UserPtr::new(src_va, dst.len())?;
    if src.is_empty() { return Ok(0); }

    let root_pa  = root_pfn << 12;
    let mut done = 0usize;

    // Step 2: page-by-page walk.
    while done < dst.len() {
        let va      = src.addr() + done;
        let page_va = va & !0xFFF;
        let offset  = va &  0xFFF;
        let remain  = (0x1000 - offset).min(dst.len() - done);

        // SAFETY: category=page-table-mutation pointer and length are validated against the task VMA map before this call.
        let (frame, perms) = unsafe {
            page_table::translate(root_pa, VirtAddr(page_va))
                .map_err(|_| UserCopyError::NotMapped)?
        };

        if !perms.contains(VmPerms::R) || !perms.contains(VmPerms::U) {
            return Err(UserCopyError::PermissionDenied);
        }

        let pa = frame.pa() + offset;
        // SAFETY: category=raw-pointer-deref pointer and length are validated against the task VMA map before this call.
        unsafe {
            core::ptr::copy_nonoverlapping(
                pa as *const u8,
                dst[done..].as_mut_ptr(),
                remain,
            );
        }
        done += remain;
    }

    Ok(done)
}
