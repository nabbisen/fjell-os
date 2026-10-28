//! Memory management error type.
#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MmError {
    /// No free frames available.
    OutOfMemory,
    /// Address range is invalid or out of bounds.
    InvalidRange,
    /// The range is already reserved.
    AlreadyReserved,
    /// Frame is not currently allocated.
    NotAllocated,
    /// Attempted to free a frame that is already free.
    DoubleFree,
    /// Address or size is not properly aligned.
    Misaligned,
    /// Virtual address is already mapped.
    AlreadyMapped,
    /// Virtual address is not mapped.
    NotMapped,
    /// Requested permission is not allowed.
    PermissionViolation,
    /// Virtual address is not canonical.
    InvalidVirtualAddress,
}
