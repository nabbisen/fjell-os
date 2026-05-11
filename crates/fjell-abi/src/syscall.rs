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
    // ── M2 syscalls ────────────────────────────────────────────────────────
    Yield      = 0,
    Exit       = 1,
    /// Write bytes to UART — smoke-test only, removed in production ABI.
    DebugWrite = 2,

    // ── M3 capability syscalls ─────────────────────────────────────────────
    /// Copy a capability within the calling task's CSpace.
    CapCopy    = 10,
    /// Copy with rights attenuation and/or badge assignment.
    CapMint    = 11,
    /// Delete a capability slot (slot becomes empty; object may survive).
    CapDelete  = 12,
    /// Delete all descendants of a capability, keeping the target.
    CapRevoke  = 13,
    /// Inspect a capability slot (debug / introspection).
    CapInspect = 14,

    // ── M3 IPC syscalls ────────────────────────────────────────────────────
    /// Synchronous send (blocks until a receiver is ready).
    IpcSend    = 20,
    /// Synchronous receive (blocks until a sender delivers).
    IpcRecv    = 21,
    /// Send + block waiting for a one-shot reply.
    IpcCall    = 22,
    /// Consume the one-shot reply edge and deliver a reply.
    IpcReply   = 23,
}

impl SyscallNumber {
    /// Decode a raw `usize` into a `SyscallNumber`, returning `None` for
    /// unknown values.
    pub fn from_usize(n: usize) -> Option<Self> {
        match n {
            0  => Some(Self::Yield),
            1  => Some(Self::Exit),
            2  => Some(Self::DebugWrite),
            10 => Some(Self::CapCopy),
            11 => Some(Self::CapMint),
            12 => Some(Self::CapDelete),
            13 => Some(Self::CapRevoke),
            14 => Some(Self::CapInspect),
            20 => Some(Self::IpcSend),
            21 => Some(Self::IpcRecv),
            22 => Some(Self::IpcCall),
            23 => Some(Self::IpcReply),
            _  => None,
        }
    }
}
