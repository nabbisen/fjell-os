//! Fjell OS — kernel / user-space ABI definitions.
//!
//! This crate defines the stable boundary between the kernel and user-space:
//! syscall numbers, error codes, and ABI-safe primitive types.  It must
//! compile in both `no_std` (kernel side) and `std` (user-space tools)
//! environments.

#![no_std]

/// Syscall numbers.
///
/// Extended in M3 with IPC and capability syscalls.
#[repr(usize)]
#[non_exhaustive]
pub enum SyscallNumber {
    Yield = 0,
    Exit = 1,
    /// Debug UART write — smoke-test only, removed in production ABI.
    DebugWrite = 2,
    // IPC / capability numbers reserved for M3.
}

/// Syscall error codes returned in `a0`.
///
/// Zero means success; negative values indicate specific errors.
#[repr(isize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SysError {
    Ok = 0,
    UnknownSyscall = -1,
    InvalidArg = -2,
    PermissionDenied = -3,
    BadState = -4,
    InternalError = -5,
}
