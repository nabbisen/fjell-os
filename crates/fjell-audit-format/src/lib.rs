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

// ── RFC 041: audit persistence pipeline types ─────────────────────────────────

/// Size of one [`AuditPersistRecord`] in bytes (40 bytes).
///
/// Extends `AuditRecordBin` (32 bytes) with 8 bytes of task identity.
pub const AUDIT_PERSIST_RECORD_SIZE: usize = 40;

/// Binary record written to storaged by `auditd` (RFC 041 §"Persistence format").
///
/// Layout (C, little-endian):
/// ```text
/// offset  size  field
///  0       8    seq           (u64)
///  8       8    tick          (u64)
/// 16       2    kind          (u16)
/// 18       2    task          (u16, 0xFFFF = no task)
/// 20       4    arg0          (u32)
/// 24       4    arg1          (u32)
/// 28       4    result        (i32)
/// 32       4    persist_seq   (u32) — write ordinal in storaged log
/// 36       4    reserved      (u32, must be 0)
/// ```
/// Total: 40 bytes.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct AuditPersistRecord {
    pub seq:          u64,
    pub tick:         u64,
    pub kind:         u16,
    pub task:         u16,
    pub arg0:         u32,
    pub arg1:         u32,
    pub result:       i32,
    /// Ordinal of this write in the storaged audit log segment.
    pub persist_seq:  u32,
    pub _reserved:    u32,
}

const _: () = assert!(
    core::mem::size_of::<AuditPersistRecord>() == AUDIT_PERSIST_RECORD_SIZE
);

impl AuditPersistRecord {
    /// Build from an `AuditRecordBin` and a storaged write ordinal.
    pub fn from_bin(bin: &AuditRecordBin, persist_seq: u32) -> Self {
        AuditPersistRecord {
            seq:         bin.seq,
            tick:        bin.tick,
            kind:        bin.kind,
            task:        bin.task,
            arg0:        bin.arg0,
            arg1:        bin.arg1,
            result:      bin.result,
            persist_seq,
            _reserved:   0,
        }
    }

    /// Extract the base `AuditRecordBin`.
    pub fn to_bin(&self) -> AuditRecordBin {
        AuditRecordBin {
            seq:    self.seq,
            tick:   self.tick,
            kind:   self.kind,
            task:   self.task,
            arg0:   self.arg0,
            arg1:   self.arg1,
            result: self.result,
        }
    }

    /// Return the `AuditKind` discriminant.
    pub fn kind(&self) -> AuditKind { AuditKind::from_u16(self.kind) }
}

/// Magic bytes for an on-disk audit log segment header (RFC 041 §"Segment header").
pub const AUDIT_LOG_MAGIC: [u8; 8] = *b"FJLAUDIT";

/// Schema version for the v0.2 audit log format.
pub const AUDIT_LOG_SCHEMA_V2: u16 = 2;

/// Header written at the start of each storaged audit log segment.
///
/// Size: 32 bytes (matches `AUDIT_RECORD_BIN_SIZE` for alignment).
#[derive(Clone, Copy)]
#[repr(C)]
pub struct AuditLogHeader {
    /// Magic identifier `FJLAUDIT`.
    pub magic:           [u8; 8],   // offset 0
    /// Sequence number of the first record in this segment.
    pub first_seq:       u64,       // offset 8
    /// Total records dropped (global, from `sys_audit_drain.a2`) at segment open.
    pub dropped_at_open: u64,       // offset 16
    /// Schema version (`AUDIT_LOG_SCHEMA_V2 = 2`).
    pub schema_version:  u16,       // offset 24
    /// Flags (reserved, must be 0).
    pub flags:           u16,       // offset 26
    /// Reserved (padding to 32 bytes).
    pub _reserved:       u32,       // offset 28
    // Total: 32 bytes
}

const _: () = assert!(
    core::mem::size_of::<AuditLogHeader>() == AUDIT_RECORD_BIN_SIZE
);

impl AuditLogHeader {
    pub fn new(first_seq: u64, dropped_at_open: u64) -> Self {
        AuditLogHeader {
            magic: AUDIT_LOG_MAGIC,
            first_seq,
            dropped_at_open,
            schema_version: AUDIT_LOG_SCHEMA_V2,
            flags: 0,
            _reserved: 0,
        }
    }
    pub fn is_valid(&self) -> bool {
        self.magic == AUDIT_LOG_MAGIC
            && self.schema_version == AUDIT_LOG_SCHEMA_V2
            && self.flags == 0
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persist_record_roundtrip() {
        let bin = AuditRecordBin {
            seq: 42, tick: 100, kind: 15 /* CapDrop */,
            task: 3, arg0: 1, arg1: 0, result: 0,
        };
        let persist = AuditPersistRecord::from_bin(&bin, 7);
        assert_eq!(persist.persist_seq, 7);
        assert_eq!(persist.kind(), AuditKind::CapDrop);
        let back = persist.to_bin();
        assert_eq!(back.seq, 42);
        assert_eq!(back.kind, 15);
    }

    #[test]
    fn audit_log_header_validity() {
        let h = AuditLogHeader::new(0, 0);
        assert!(h.is_valid());
        let bad_magic = AuditLogHeader { magic: *b"BADBADBA", ..h };
        assert!(!bad_magic.is_valid());
    }

    #[test]
    fn audit_kind_v2_labels() {
        assert_eq!(AuditKind::CapDrop.label(), "cap.drop");
        assert_eq!(AuditKind::LeaseRevoked.label(), "lease.revoked");
        assert_eq!(AuditKind::TaskQuantumExceeded.label(), "task.quantum_exceeded");
    }

    #[test]
    fn persist_record_size_check() {
        assert_eq!(
            core::mem::size_of::<AuditPersistRecord>(),
            AUDIT_PERSIST_RECORD_SIZE
        );
    }

    #[test]
    fn audit_log_header_size_check() {
        assert_eq!(
            core::mem::size_of::<AuditLogHeader>(),
            AUDIT_RECORD_BIN_SIZE
        );
    }
}
