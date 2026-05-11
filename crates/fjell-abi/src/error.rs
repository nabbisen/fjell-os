//! Syscall error codes returned in register `a0`.

/// Syscall return status.
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

impl SysError {
    /// Convert a raw `isize` return value into `SysError`.
    pub fn from_isize(v: isize) -> Self {
        match v {
            0 => Self::Ok,
            -1 => Self::UnknownSyscall,
            -2 => Self::InvalidArg,
            -3 => Self::PermissionDenied,
            -4 => Self::BadState,
            _ => Self::InternalError,
        }
    }
}
