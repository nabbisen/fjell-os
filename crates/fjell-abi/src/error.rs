//! Syscall error codes returned in register `a0`.

/// Syscall return status.
///
/// Zero means success; negative values indicate specific errors.
#[repr(isize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SysError {
    Ok                = 0,
    // ── General errors ─────────────────────────────────────────────────────
    UnknownSyscall    = -1,
    InvalidArg        = -2,
    PermissionDenied  = -3,
    BadState          = -4,
    InternalError     = -5,
    // ── Capability errors ──────────────────────────────────────────────────
    /// Handle is invalid or generation has been recycled.
    InvalidCap        = -10,
    /// Capability kind does not match the expected kind.
    WrongType         = -11,
    /// The referenced slot is empty.
    SlotEmpty         = -12,
    /// The destination slot is already occupied.
    SlotOccupied      = -13,
    /// Attempted to grant more rights than the source capability holds.
    RightsExceed      = -14,
    /// Capability transfer is not permitted by the source rights.
    CapTransferDenied = -15,
    // ── IPC errors ─────────────────────────────────────────────────────────
    /// Non-blocking operation would have blocked.
    WouldBlock        = -20,
    /// Endpoint send or receive queue is full.
    QueueFull         = -21,
    /// Message exceeds the maximum word or cap count.
    MsgTooLong        = -22,
    /// Waiting endpoint or reply edge was cancelled.
    Canceled          = -23,
    // ── Resource errors ────────────────────────────────────────────────────
    NoMemory          = -30,
    AlreadyMapped     = -31,
    NotMapped         = -32,
    InvalidAddress    = -33,
    NotSupported      = -34,
    // ── Lease / v0.2 errors ────────────────────────────────────────────────
    /// The capability's lease has been revoked (epoch mismatch or state Revoked).
    LeaseRevoked      = -40,
    /// Lease not yet active or epoch check invalid.
    LeaseExpired      = -41,
    /// Handle generation does not match the slot's current generation.
    GenerationMismatch = -42,
}

impl SysError {
    pub fn from_isize(v: isize) -> Self {
        match v {
            0   => Self::Ok,
            -1  => Self::UnknownSyscall,
            -2  => Self::InvalidArg,
            -3  => Self::PermissionDenied,
            -4  => Self::BadState,
            -10 => Self::InvalidCap,
            -11 => Self::WrongType,
            -12 => Self::SlotEmpty,
            -13 => Self::SlotOccupied,
            -14 => Self::RightsExceed,
            -15 => Self::CapTransferDenied,
            -20 => Self::WouldBlock,
            -21 => Self::QueueFull,
            -22 => Self::MsgTooLong,
            -23 => Self::Canceled,
            -30 => Self::NoMemory,
            -31 => Self::AlreadyMapped,
            -32 => Self::NotMapped,
            -33 => Self::InvalidAddress,
            -34 => Self::NotSupported,
            -40 => Self::LeaseRevoked,
            -41 => Self::LeaseExpired,
            -42 => Self::GenerationMismatch,
            _   => Self::InternalError,
        }
    }
}
