//! Safe user-pointer validation (RFC 039 §2.1).
//!
//! # Purpose
//!
//! `copy_to_user` and `copy_from_user` are the highest-risk primitives in
//! the kernel.  This module provides a validated `UserPtr` type that rejects
//! obviously-invalid pointer ranges *before* the page-table walk.
//!
//! Validation is pure arithmetic — no page-table access — so the checks can
//! be property-tested on the host.  The page-table walk happens in
//! `user_copy.rs` and validates the remaining invariants (mapped, permission).
//!
//! # Required rejections (RFC 039 §2.1)
//!
//! ```text
//! 1. null pointer (addr == 0)
//! 2. kernel address (addr >= RAM_BASE)
//! 3. length overflow / wraparound
//! 4. range end in kernel (addr + len > RAM_BASE)
//! 5. range crosses RAM_BASE from below
//! ```
//!
//! Per-page mapped/writable/user checks happen inside `copy_to_user_bytes`.

/// Base of kernel physical RAM (QEMU virt).
///
/// Any user address ≥ this is in kernel territory and must be rejected.
/// This constant is replicated here to keep the pure-logic module self-contained.
pub const USER_ADDR_MAX: usize = crate::platform::qemu_virt::RAM_BASE;

/// Error type for all `copy_to_user` / `copy_from_user` failures.
///
/// Maps to `SysError::InvalidArg` or `SysError::PermissionDenied` at the
/// syscall boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserCopyError {
    /// Pointer is null (addr == 0).
    NullPointer,
    /// Pointer is in kernel address range (addr ≥ RAM_BASE).
    KernelAddress,
    /// `addr + len` overflows `usize`.
    LengthOverflow,
    /// The range `[addr, addr+len)` crosses or ends in the kernel address range.
    RangeCrossesKernel,
    /// A page in the destination/source range is not mapped.
    NotMapped,
    /// A mapped page lacks the required permissions (W|U for write, R|U for read).
    PermissionDenied,
    /// Internal kernel error (e.g. no page table found).
    Internal,
}

impl From<UserCopyError> for fjell_abi::error::SysError {
    fn from(e: UserCopyError) -> Self {
        use fjell_abi::error::SysError;
        match e {
            UserCopyError::NullPointer          => SysError::InvalidArg,
            UserCopyError::KernelAddress        => SysError::InvalidArg,
            UserCopyError::LengthOverflow       => SysError::InvalidArg,
            UserCopyError::RangeCrossesKernel   => SysError::InvalidArg,
            UserCopyError::NotMapped            => SysError::InvalidAddress,
            UserCopyError::PermissionDenied     => SysError::PermissionDenied,
            UserCopyError::Internal             => SysError::InternalError,
        }
    }
}

/// A user-space virtual address validated to be a plausible user address.
///
/// Constructing a `UserPtr` does NOT guarantee the address is mapped or
/// writable — those checks happen in the page-table walk.  It guarantees
/// only that the address is in the canonical user address range.
#[derive(Clone, Copy, Debug)]
pub struct UserPtr {
    addr: usize,
    len:  usize,
}

impl UserPtr {
    /// Validate `addr..addr+len` as a user-space range.
    ///
    /// Returns `Err` if:
    /// - addr is 0 (null pointer)
    /// - addr ≥ `USER_ADDR_MAX` (kernel address)
    /// - addr + len overflows `usize`
    /// - addr + len > `USER_ADDR_MAX` (range ends in kernel)
    ///
    /// A zero-length range returns `Ok` — the caller decides whether to
    /// treat it as a no-op.
    pub fn new(addr: usize, len: usize) -> Result<Self, UserCopyError> {
        // 1. Null pointer.
        if addr == 0 {
            return Err(UserCopyError::NullPointer);
        }
        // 2. Kernel address (starts in kernel space).
        if addr >= USER_ADDR_MAX {
            return Err(UserCopyError::KernelAddress);
        }
        // 3. Length overflow (addr + len wraps around).
        let end = addr.checked_add(len).ok_or(UserCopyError::LengthOverflow)?;
        // 4. Range end in kernel space.
        if end > USER_ADDR_MAX {
            return Err(UserCopyError::RangeCrossesKernel);
        }
        Ok(UserPtr { addr, len })
    }

    /// The validated base address.
    #[inline] pub fn addr(self) -> usize { self.addr }
    /// The length in bytes.
    #[allow(dead_code)]  // part of UserPtr API; used by copy_from_user_bytes callers
    #[inline] pub fn len(self) -> usize { self.len }
    /// True if the range is empty.
    #[inline] pub fn is_empty(self) -> bool { self.len == 0 }
}

// ── Host-side property tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in for RAM_BASE in tests.  RFC 039 §"Fuzz corpus" cases:
    const KBASE: usize = USER_ADDR_MAX;

    #[test]
    fn null_pointer_rejected() {
        assert_eq!(UserPtr::new(0, 16).unwrap_err(), UserCopyError::NullPointer);
    }

    #[test]
    fn zero_len_null_still_rejected() {
        // Even a zero-length null ptr must be rejected.
        assert_eq!(UserPtr::new(0, 0).unwrap_err(), UserCopyError::NullPointer);
    }

    #[test]
    fn kernel_address_rejected() {
        assert_eq!(
            UserPtr::new(KBASE, 1).unwrap_err(),
            UserCopyError::KernelAddress
        );
        assert_eq!(
            UserPtr::new(KBASE + 0x1000, 1).unwrap_err(),
            UserCopyError::KernelAddress
        );
        assert_eq!(
            UserPtr::new(usize::MAX, 1).unwrap_err(),
            UserCopyError::KernelAddress
        );
    }

    #[test]
    fn length_overflow_rejected() {
        // addr + len wraps around: usize::MAX + 1 overflows.
        let addr = KBASE - 8;
        assert_eq!(
            UserPtr::new(addr, usize::MAX).unwrap_err(),
            UserCopyError::LengthOverflow
        );
    }

    #[test]
    fn range_crosses_kernel_rejected() {
        // Start is valid but end crosses RAM_BASE.
        let addr = KBASE - 1;
        assert_eq!(
            UserPtr::new(addr, 2).unwrap_err(),
            UserCopyError::RangeCrossesKernel
        );
        // End exactly at KBASE is also rejected (strict <).
        assert_eq!(
            UserPtr::new(addr, 1).unwrap_err(),
            UserCopyError::RangeCrossesKernel
        );
    }

    #[test]
    fn valid_user_range_accepted() {
        // Well inside user space.
        let p = UserPtr::new(0x1000, 64).unwrap();
        assert_eq!(p.addr(), 0x1000);
        assert_eq!(p.len(), 64);
    }

    #[test]
    fn zero_len_valid_address_accepted() {
        // Zero-length at a valid address is accepted; caller decides semantics.
        let p = UserPtr::new(0x1000, 0).unwrap();
        assert!(p.is_empty());
    }

    #[test]
    fn end_just_below_kernel_accepted() {
        // End = KBASE - 1 is still user space.
        let addr = KBASE - 8;
        let p = UserPtr::new(addr, 7).unwrap();
        assert_eq!(p.addr(), addr);
        assert_eq!(p.len(), 7);
    }

    #[test]
    fn partially_valid_range_crosses_kernel() {
        // RFC 039 §"partially valid range": addr is valid, addr+len is kernel.
        let addr = KBASE / 2;
        let len  = KBASE / 2 + 1;   // addr + len > KBASE
        assert_eq!(
            UserPtr::new(addr, len).unwrap_err(),
            UserCopyError::RangeCrossesKernel
        );
    }
}
