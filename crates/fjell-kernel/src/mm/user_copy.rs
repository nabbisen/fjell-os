//! Safe byte-level copy from kernel to user address space.
//!
//! The kernel uses an identity map (PA == kernel VA), so once we have the
//! physical address of a user page we can write to it directly.

use super::address::VirtAddr;
use super::vspace::VmPerms;
use super::page_table;

/// Error type for user-copy operations.
#[derive(Debug)]
pub enum UserCopyError {
    /// `dst_va` is not in user-space range (must be < RAM_BASE).
    NotUserAddress,
    /// A page in the destination range is not mapped in the task's address space.
    NotMapped,
    /// A page in the destination range lacks the required permissions (W | U).
    PermissionDenied,
}

/// Copy `src` bytes into the user address space described by `root_pfn`.
///
/// `dst_va` must be a user-space virtual address (< `RAM_BASE`).
/// Translates page by page via [`page_table::translate`] and writes
/// through the kernel identity map (PA == VA).
///
/// Returns the number of bytes successfully written, or an error on the
/// first unmapped or non-writable page.
///
/// # Safety
/// `root_pfn` must be the valid Sv39 root PFN for the target task's address
/// space and must remain live for the duration of this call.
pub unsafe fn copy_to_user_bytes(
    root_pfn: usize,
    dst_va:   usize,
    src:      &[u8],
) -> Result<usize, UserCopyError> {
    use crate::platform::qemu_virt::RAM_BASE;

    if dst_va == 0 || dst_va >= RAM_BASE {
        return Err(UserCopyError::NotUserAddress);
    }
    if dst_va.saturating_add(src.len()) > RAM_BASE {
        return Err(UserCopyError::NotUserAddress);
    }

    let root_pa  = root_pfn << 12;
    let mut done = 0usize;

    while done < src.len() {
        let va      = dst_va + done;
        let page_va = va & !0xFFF;
        let offset  = va &  0xFFF;
        let remain  = (0x1000 - offset).min(src.len() - done);

        let (frame, perms) = unsafe {
            page_table::translate(root_pa, VirtAddr(page_va))
                .map_err(|_| UserCopyError::NotMapped)?
        };

        if !perms.contains(VmPerms::W) || !perms.contains(VmPerms::U) {
            return Err(UserCopyError::PermissionDenied);
        }

        let pa = frame.pa() + offset;
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
