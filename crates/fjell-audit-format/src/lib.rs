//! Audit event schema for Fjell OS.
//!
//! Defines the canonical `AuditEvent` type used both inside the kernel's
//! fixed-capacity ring and by the user-space `auditd` service.
//!
//! # Binary wire format
//!
//! `sys_audit_drain` copies [`AuditRecordBin`] structs into the caller's
//! buffer.  Each record is exactly [`AUDIT_RECORD_BIN_SIZE`] bytes.
//! The layout is `#[repr(C)]` and stable across builds for a given Fjell
//! version.

#![no_std]

// ── Kind discriminant ────────────────────────────────────────────────────────

/// Discriminant for each auditable kernel action.
///
/// **Stability**: values are assigned to match [`AuditKindBin`] in the wire
/// format.  Do not reorder without bumping the format version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AuditKind {
    Boot            = 0,
    VmMap           = 1,
    VmFault         = 2,
    TaskCreate      = 3,
    TaskSwitch      = 4,
    TaskExit        = 5,
    TaskFault       = 6,
    Syscall         = 7,
    UnknownSyscall  = 8,
    // M3 capability / IPC events
    CapCopy         = 10,
    CapMint         = 11,
    CapDelete       = 12,
    CapRevoke       = 13,
    // v0.2 capability events (RFC 031–036)
    CapDrop         = 15,
    LeaseRevoked    = 16,
    IpcSend         = 20,
    IpcRecv         = 21,
    IpcCall         = 22,
    IpcReply        = 23,
    IpcDenied       = 24,
    // v0.2 scheduler events (RFC 037)
    TaskQuantumExceeded = 30,
    // Internal / unclassified
    Internal        = 255,
}

impl AuditKind {
    /// Convert the wire-format `u16` discriminant back to `AuditKind`.
    pub fn from_u16(v: u16) -> Self {
        match v {
            0  => Self::Boot,
            1  => Self::VmMap,
            2  => Self::VmFault,
            3  => Self::TaskCreate,
            4  => Self::TaskSwitch,
            5  => Self::TaskExit,
            6  => Self::TaskFault,
            7  => Self::Syscall,
            8  => Self::UnknownSyscall,
            10 => Self::CapCopy,
            11 => Self::CapMint,
            12 => Self::CapDelete,
            13 => Self::CapRevoke,
            15 => Self::CapDrop,
            16 => Self::LeaseRevoked,
            20 => Self::IpcSend,
            21 => Self::IpcRecv,
            22 => Self::IpcCall,
            23 => Self::IpcReply,
            24 => Self::IpcDenied,
            30 => Self::TaskQuantumExceeded,
            _  => Self::Internal,
        }
    }

    /// Short ASCII label used in JSON Lines output.
    pub fn label(self) -> &'static str {
        match self {
            Self::Boot               => "boot",
            Self::VmMap              => "vm.map",
            Self::VmFault            => "vm.fault",
            Self::TaskCreate         => "task.create",
            Self::TaskSwitch         => "task.switch",
            Self::TaskExit           => "task.exit",
            Self::TaskFault          => "task.fault",
            Self::Syscall            => "syscall",
            Self::UnknownSyscall     => "unknown_syscall",
            Self::CapCopy            => "cap.copy",
            Self::CapMint            => "cap.mint",
            Self::CapDelete          => "cap.delete",
            Self::CapRevoke          => "cap.revoke",
            Self::CapDrop            => "cap.drop",
            Self::LeaseRevoked       => "lease.revoked",
            Self::IpcSend            => "ipc.send",
            Self::IpcRecv            => "ipc.recv",
            Self::IpcCall            => "ipc.call",
            Self::IpcReply           => "ipc.reply",
            Self::IpcDenied          => "ipc.denied",
            Self::TaskQuantumExceeded=> "task.quantum_exceeded",
            Self::Internal           => "internal",
        }
    }
}

// ── Logical record (used inside fjell-audit-format consumers) ────────────────

/// A single audit record stored in the kernel ring (logical view).
#[derive(Clone, Copy, Debug)]
pub struct AuditEvent {
    /// Monotonically increasing sequence number.
    pub seq:    u64,
    /// Kernel tick at which the event was recorded (currently 0 in M7).
    pub tick:   u64,
    /// Task index, if the event is task-scoped.
    pub task:   Option<u16>,
    pub kind:   AuditKind,
    pub arg0:   usize,
    pub arg1:   usize,
    /// Zero on success; negative `SysError` value on failure.
    pub result: isize,
}

// ── Stable binary wire format ─────────────────────────────────────────────────

/// Size of one [`AuditRecordBin`] in bytes (32 bytes).
pub const AUDIT_RECORD_BIN_SIZE: usize = 32;

/// Flat binary representation of one audit record as drained by
/// `sys_audit_drain`.
///
/// Layout (C, little-endian):
/// ```text
/// offset  size  field
///  0       8    seq    (u64)
///  8       8    tick   (u64)
/// 16       2    kind   (u16, see AuditKind discriminant)
/// 18       2    task   (u16, 0xFFFF = no task)
/// 20       4    arg0   (u32)
/// 24       4    arg1   (u32)
/// 28       4    result (i32)
/// ```
/// Total: 32 bytes.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct AuditRecordBin {
    pub seq:    u64,
    pub tick:   u64,
    pub kind:   u16,
    /// Task slot index, or `0xFFFF` if not task-scoped.
    pub task:   u16,
    pub arg0:   u32,
    pub arg1:   u32,
    pub result: i32,
}

// Compile-time size check.
const _: () = assert!(core::mem::size_of::<AuditRecordBin>() == AUDIT_RECORD_BIN_SIZE);

impl AuditRecordBin {
    /// Parse the canonical binary record from a 32-byte slice.
    /// Returns `None` if `bytes.len() < 32`.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < AUDIT_RECORD_BIN_SIZE { return None; }
        Some(Self {
            seq:    u64::from_le_bytes(bytes[0..8].try_into().ok()?),
            tick:   u64::from_le_bytes(bytes[8..16].try_into().ok()?),
            kind:   u16::from_le_bytes(bytes[16..18].try_into().ok()?),
            task:   u16::from_le_bytes(bytes[18..20].try_into().ok()?),
            arg0:   u32::from_le_bytes(bytes[20..24].try_into().ok()?),
            arg1:   u32::from_le_bytes(bytes[24..28].try_into().ok()?),
            result: i32::from_le_bytes(bytes[28..32].try_into().ok()?),
        })
    }

    /// Return the `AuditKind` discriminant.
    pub fn kind(self) -> AuditKind { AuditKind::from_u16(self.kind) }
}
