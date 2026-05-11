//! Syscall numbers for the Fjell OS ABI.
//!
//! Calling convention:
//!   a7 = syscall number
//!   a0–a5 = arguments
//!   a0 (return) = SysError code (0 = Ok, negative = error)
//!   a1–a3 (return) = optional return values on success
//!
//! After `ecall`, `sepc` is advanced by 4 before `sret`.

/// Syscall numbers.
#[repr(usize)]
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyscallNumber {
    Yield = 0,
    Exit = 1,
    /// Write bytes to the UART console.
    /// Smoke-test aid only — will be removed or capability-protected in M3+.
    DebugWrite = 2,
    // IPC and capability numbers reserved for M3.
}

impl SyscallNumber {
    /// Try to decode a raw `usize` into a `SyscallNumber`.
    pub fn from_usize(n: usize) -> Option<Self> {
        match n {
            0 => Some(Self::Yield),
            1 => Some(Self::Exit),
            2 => Some(Self::DebugWrite),
            _ => None,
        }
    }
}
