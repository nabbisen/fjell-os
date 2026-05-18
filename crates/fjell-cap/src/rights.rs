//! Capability rights bitmask, kind discriminant, object scope, and error types.
//!
//! All types in this module are from RFC 031 §2.1–§2.7 (v0.2.0).
//!
//! # Backward-compatibility note
//! `CapRights` was extended from `u32` to `u64` in v0.2.0 (RFC 031 §2.1).
//! Service crates that hardcode raw `u32` constants must be migrated to the
//! named constants in this module.

use fjell_abi::task::TaskId;
use fjell_abi::lease::LeaseId;

// ── CapRights ─────────────────────────────────────────────────────────────────

/// Permission bits attached to a capability (RFC 031 §2.1).
///
/// Invariant CAP-A: `child.rights ⊆ parent.rights` must hold after any
/// `cap_copy`, `cap_mint`, or delegation operation.
///
/// Rights must be checked independently of kind; a correct kind alone is
/// insufficient to authorise an operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct CapRights(pub u64);

#[allow(clippy::unusual_byte_groupings)]
impl CapRights {
    // Memory / mapping rights
    pub const READ:          Self = CapRights(1 << 0);
    pub const WRITE:         Self = CapRights(1 << 1);
    pub const EXECUTE:       Self = CapRights(1 << 2);

    // IPC rights
    pub const SEND:          Self = CapRights(1 << 3);
    pub const RECV:          Self = CapRights(1 << 4);
    pub const CALL:          Self = CapRights(1 << 5);
    pub const REPLY:         Self = CapRights(1 << 6);

    // Capability management rights
    pub const COPY:          Self = CapRights(1 << 7);
    pub const MINT:          Self = CapRights(1 << 8);
    pub const REVOKE:        Self = CapRights(1 << 9);
    pub const INSPECT:       Self = CapRights(1 << 10);
    pub const DROP:          Self = CapRights(1 << 11);

    // Task management rights
    pub const TASK_CREATE:   Self = CapRights(1 << 12);
    pub const TASK_START:    Self = CapRights(1 << 13);
    pub const TASK_STATUS:   Self = CapRights(1 << 14);
    pub const TASK_KILL:     Self = CapRights(1 << 15);

    // Lease management rights
    pub const LEASE_CREATE:  Self = CapRights(1 << 16);
    pub const LEASE_REVOKE:  Self = CapRights(1 << 17);
    pub const LEASE_INSPECT: Self = CapRights(1 << 18);

    // Device rights
    pub const MMIO_MAP:      Self = CapRights(1 << 19);
    pub const DMA_ALLOC:     Self = CapRights(1 << 20);
    pub const DMA_USE:       Self = CapRights(1 << 21);
    pub const DMA_REVOKE:    Self = CapRights(1 << 22);

    // System rights
    pub const AUDIT_DRAIN:   Self = CapRights(1 << 23);
    pub const BOOT_READ:     Self = CapRights(1 << 24);
    pub const REBOOT:        Self = CapRights(1 << 25);
    /// RFC 056: authority to call `sys_cap_install` (meta-right; bit 26).
    pub const CAP_INSTALL:   Self = CapRights(1 << 26);

    /// All defined rights.
    pub const ALL:  Self = CapRights((1 << 26) - 1);
    /// No rights.
    pub const NONE: Self = CapRights(0);

    /// Returns `true` if `self` contains all bits in `required`.
    #[inline]
    pub fn contains(self, required: CapRights) -> bool {
        self.0 & required.0 == required.0
    }

    /// Returns `true` if `self` rights are a subset of `parent` rights.
    ///
    /// Enforces invariant CAP-A: used by `cap_mint` and `require_cap`.
    #[inline]
    pub fn is_subset_of(self, parent: CapRights) -> bool {
        self.0 & !parent.0 == 0
    }
}

impl core::ops::BitOr for CapRights {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self { CapRights(self.0 | rhs.0) }
}

impl core::ops::BitAnd for CapRights {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self { CapRights(self.0 & rhs.0) }
}

impl core::ops::BitOrAssign for CapRights {
    fn bitor_assign(&mut self, rhs: Self) { self.0 |= rhs.0; }
}

// ── CapKind ───────────────────────────────────────────────────────────────────

/// Discriminant for the kind of kernel object a capability references
/// (RFC 031 §2.2).
///
/// Every authority-bearing syscall specifies a required `CapKind`.
/// A capability of the wrong kind is rejected before rights are checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapKind {
    // ── M3 (microkernel core) ───────────────────────────────────────────────
    /// An IPC synchronous rendezvous endpoint.
    Endpoint,
    /// An implicit one-shot reply edge installed by `ipc_call`.
    Reply,
    /// Authority to call `task_start`, `task_status`, `task_kill`.
    TaskControl,
    /// Authority to call `task_spawn`.
    TaskCreate,
    /// Authority to call `task_status` read-only.
    TaskInspect,
    /// Authority to call `lease_create`, `lease_revoke`, `lease_inspect`.
    LeaseAdmin,

    // ── M6 (devices) ───────────────────────────────────────────────────────
    /// A bounded MMIO physical region (RFC 031 + RFC 035).
    MmioRegion,
    /// A per-task DMA physical region (RFC 031 + RFC 036).
    DmaRegion,

    // ── M4 (audit) ─────────────────────────────────────────────────────────
    /// Authority to call `audit_drain` (RFC 031 + RFC 039).
    AuditDrain,

    // ── M8 (evidence) ──────────────────────────────────────────────────────
    /// Authority to call `boot_evidence_get` (read-only).
    BootEvidence,
    /// Authority to call `reboot`.
    Reboot,
    /// RFC 056: authority to install caps into other tasks' CSpaces.
    /// Granted only to cap-broker and init during bootstrap.
    CapInstall,
    /// Authority over the persistent state store namespace.
    PersistentStore,
    /// Authority over boot-control operations.
    BootControl,
    /// Authority over upgrade transactions.
    UpgradeTransaction,
    /// Authority to perform rootfs / bundle verification.
    Verification,
    /// Read-only access to the immutable rootfs namespace.
    RootfsRead,
    /// Authority to create a system snapshot.
    SnapshotCreate,
    /// Authority to read existing snapshots.
    SnapshotRead,

    // ── Backward-compatible aliases (v0.1.x names) ─────────────────────────
    // These are kept for the transition period; service code should migrate.
    /// Legacy: use `DmaRegion` for new code.
    DmaAlloc,
    /// A virtual address space (unused in v0.2 syscall paths).
    AddressSpace,
    /// A physical memory frame.
    Frame,
    /// A task (direct task reference) — distinct from `TaskControl`.
    Task,
}

// ── ObjectScope ───────────────────────────────────────────────────────────────

/// Object-level scope that bounds what a capability can act on
/// (RFC 031 §2.3).
///
/// Initial v0.2 implementation supports `Any`, `Task`, `Endpoint`,
/// `Lease`, `MmioRegion`, and `DmaRegion`.
/// Other variants are accepted by the ABI but the scope check is a no-op
/// until the relevant RFC lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectScope {
    /// No scope restriction — capability acts on any object of its kind.
    Any,
    /// Scoped to a specific object by raw id.
    Object(u32),
    /// Scoped to a specific task.
    Task(TaskId),
    /// Scoped to a specific IPC endpoint.
    Endpoint(u32),
    /// Scoped to a specific lease.
    Lease(LeaseId),
    /// Scoped to a specific MMIO region (RFC 035).
    MmioRegion(u32),
    /// Scoped to a specific DMA region (RFC 036).
    DmaRegion(u32),
    /// Scoped to a persistent-store namespace.
    StoreNamespace(u32),
    /// Scoped to a specific boot slot.
    BootSlot(u8),
}

impl ObjectScope {
    /// Returns `true` if this scope is compatible with the requested scope.
    ///
    /// `self` is the scope stored in the capability; `requested` is the
    /// scope asserted by the caller.
    ///
    /// Rules (RFC 031 §2.3):
    /// - `Any` is compatible with everything (no restriction).
    /// - Two identical scopes are compatible.
    /// - Any other combination is a scope mismatch.
    pub fn is_satisfied_by(&self, requested: &ObjectScope) -> bool {
        match self {
            ObjectScope::Any => true,
            other => other == requested,
        }
    }
}

impl CapKind {
    /// Convert a raw u8 discriminant to a `CapKind`.  Returns `Endpoint` for unknown.
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0  => Some(Self::Endpoint),
            1  => Some(Self::Endpoint),
            2  => Some(Self::TaskControl),
            3  => Some(Self::TaskCreate),
            4  => Some(Self::LeaseAdmin),
            5  => Some(Self::MmioRegion),
            6  => Some(Self::DmaRegion),
            7  => Some(Self::DmaRegion),
            8  => Some(Self::AuditDrain),
            9  => Some(Self::BootEvidence),
            10 => Some(Self::Reboot),
            16 => Some(Self::CapInstall),
            _  => None,
        }
    }
}

// ── CapState ──────────────────────────────────────────────────────────────────

/// Lifecycle state of a capability object (RFC 031 §2.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapState {
    /// The capability is present and usable.
    Active,
    /// The capability has been explicitly dropped (slot pending reuse).
    Dropped,
    /// The capability's lease has been revoked (lazy invalidation).
    ///
    /// Note: in v0.2, revocation is detected lazily via epoch mismatch in
    /// `require_cap()` rather than eagerly setting this field.  This field
    /// is reserved for future eager-revoke support.
    Revoked,
}

// ── CapError ──────────────────────────────────────────────────────────────────

/// Errors returned by `require_cap()` and the capability management functions
/// (RFC 031 §2.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapError {
    // ── lookup / generation errors ──────────────────────────────────────────
    /// Handle index is out of range or is the null handle.
    InvalidHandle,
    /// The handle's generation does not match the slot's current generation.
    GenerationMismatch,
    /// The slot is empty (no capability installed).
    EmptySlot,
    /// The capability has been explicitly dropped.
    Dropped,
    /// The capability has been revoked.
    Revoked,
    // ── enforcement errors ──────────────────────────────────────────────────
    /// The capability's kind does not match the expected kind.
    WrongKind,
    /// The capability does not carry a required right.
    MissingRight,
    /// The capability's scope does not match the required scope.
    ScopeMismatch,
    // ── lease errors ────────────────────────────────────────────────────────
    /// The capability's lease epoch does not match the current epoch.
    LeaseRevoked,
    /// The capability's lease has expired (e.g. time-bounded lease).
    LeaseExpired,
    /// The `LeaseId` generation does not match (stale lease handle).
    LeaseGenerationMismatch,
    // ── other ───────────────────────────────────────────────────────────────
    /// Internal kernel error (should not occur in correct code).
    Internal,
}

impl CapError {
    /// Map to the stable `SysError` code visible at the syscall ABI boundary
    /// (RFC 031 §2.7 error mapping table).
    pub fn to_sys_error(self) -> fjell_abi::error::SysError {
        use fjell_abi::error::SysError;
        match self {
            CapError::InvalidHandle         => SysError::InvalidCap,
            CapError::GenerationMismatch    => SysError::GenerationMismatch,
            CapError::EmptySlot             => SysError::SlotEmpty,
            CapError::Dropped               => SysError::InvalidCap,
            CapError::Revoked               => SysError::InvalidCap,
            CapError::WrongKind             => SysError::WrongType,
            CapError::MissingRight          => SysError::PermissionDenied,
            CapError::ScopeMismatch         => SysError::PermissionDenied,
            CapError::LeaseRevoked          => SysError::LeaseRevoked,
            CapError::LeaseExpired          => SysError::LeaseExpired,
            CapError::LeaseGenerationMismatch => SysError::InvalidCap,
            CapError::Internal              => SysError::InternalError,
        }
    }
}

// ── Conversions ───────────────────────────────────────────────────────────────

impl From<CapError> for fjell_abi::error::SysError {
    /// Map `CapError` to the stable `SysError` ABI code.
    ///
    /// Used in kernel syscall handlers that return `SysError` and internally
    /// call `require_cap()` / `check_lease()` which now return `CapError`.
    fn from(e: CapError) -> Self {
        e.to_sys_error()
    }
}
