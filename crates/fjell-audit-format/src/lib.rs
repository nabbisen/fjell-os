//! Audit event schema for Fjell OS.
//!
//! Defines the canonical `AuditEvent` type used both inside the kernel's
//! fixed-capacity ring and by the user-space `auditd` service.
//!
//! The format is intentionally simple: all fields are fixed-width or
//! bounded so that the kernel can record events without heap allocation.

#![no_std]

/// Discriminant for each auditable kernel action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AuditKind {
    Boot,
    VmMap,
    VmFault,
    TaskCreate,
    TaskSwitch,
    TaskExit,
    TaskFault,
    Syscall,
    UnknownSyscall,
    // IPC and capability kinds added in M3.
}

/// A single audit record stored in the kernel ring.
#[derive(Clone, Copy, Debug)]
pub struct AuditEvent {
    /// Monotonically increasing sequence number.
    pub seq: u64,
    /// Kernel tick at which the event was recorded.
    pub tick: u64,
    /// Task index, if the event is task-scoped.
    pub task: Option<u16>,
    pub kind: AuditKind,
    pub arg0: usize,
    pub arg1: usize,
    /// Zero on success; negative `SysError` value on failure.
    pub result: isize,
}
