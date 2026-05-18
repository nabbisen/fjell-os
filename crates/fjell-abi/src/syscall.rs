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
    /// Explicitly drop a capability slot, freeing it for reuse (RFC 032 / v0.2).
    ///
    /// Unlike `CapDelete`, this succeeds even when the capability's lease has
    /// been revoked.  Existence check (ownership + generation) still applies.
    CapDrop    = 15,

    // ── M3 IPC syscalls ────────────────────────────────────────────────────
    /// Synchronous send (blocks until a receiver is ready).
    IpcSend    = 20,
    /// Synchronous receive (blocks until a sender delivers).
    IpcRecv    = 21,
    /// Send + block waiting for a one-shot reply.
    IpcCall    = 22,
    /// Consume the one-shot reply edge and deliver a reply.
    IpcReply    = 23,
    /// Non-blocking IPC recv — returns WouldBlock if no message pending.
    IpcTryRecv  = 24,

    // ── M4 task-spawn syscalls ─────────────────────────────────────────────
    /// Spawn a new task from a named embedded image; returns task_handle.
    TaskSpawn  = 40,
    /// Make a Spawned task Runnable (first-time start).
    TaskStart  = 41,
    /// Query a task's current state.
    TaskStatus = 42,
    /// Terminate a running task (optional M4).
    TaskKill   = 43,

    // ── M4 lease syscalls ──────────────────────────────────────────────────
    /// Create a new lease; returns LeaseId packed into a0.
    LeaseCreate  = 50,
    /// Revoke a lease (increment epoch); invalidates all bound capabilities.
    LeaseRevoke  = 51,
    /// Inspect a lease's current epoch.
    LeaseInspect = 52,

    // ── M4 audit syscalls ──────────────────────────────────────────────────
    /// Copy pending audit records from the kernel ring into user buffer.
    AuditDrain = 60,
    // ── M6: device / MMIO / DMA primitives ─────────────────────────────────
    PlatformInfoGet = 80,
    MmioMap         = 90,
    MmioUnmap       = 91,
    IrqBind         = 100,
    IrqAck          = 101,
    DmaAlloc        = 110,
    DmaShare        = 111,
    DmaRevoke       = 112,
    Reboot          = 120,
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
            15 => Some(Self::CapDrop),
            20 => Some(Self::IpcSend),
            21 => Some(Self::IpcRecv),
            22 => Some(Self::IpcCall),
            23 => Some(Self::IpcReply),
            24 => Some(Self::IpcTryRecv),
            40 => Some(Self::TaskSpawn),
            41 => Some(Self::TaskStart),
            42 => Some(Self::TaskStatus),
            43 => Some(Self::TaskKill),
            50 => Some(Self::LeaseCreate),
            51 => Some(Self::LeaseRevoke),
            52 => Some(Self::LeaseInspect),
            60 => Some(Self::AuditDrain),
            80 => Some(Self::PlatformInfoGet),
            90 => Some(Self::MmioMap),
            91 => Some(Self::MmioUnmap),
            100 => Some(Self::IrqBind),
            101 => Some(Self::IrqAck),
            110 => Some(Self::DmaAlloc),
            111 => Some(Self::DmaShare),
            112 => Some(Self::DmaRevoke),
            120 => Some(Self::Reboot),
            _  => None,
        }
    }
}