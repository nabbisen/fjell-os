//! Capability rights bitmask and object kind discriminant.

/// Discriminant for the kind of kernel object a capability references.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapKind {
    /// A task (thread of execution with its own CSpace and address space).
    Task,
    /// A virtual address space.
    AddressSpace,
    /// An IPC synchronous rendezvous endpoint.
    Endpoint,
    /// A physical memory frame (for user-visible mapping, M4+).
    Frame,
    /// An implicit one-shot reply edge installed by `ipc_call`.
    Reply,
    // ── M7.1 additions (RFC 004) ─────────────────────────────────────────
    /// Authority to call `sys_task_spawn`.  Only init and service-manager
    /// should hold this capability.
    TaskCreate,
    /// Authority to call `sys_task_start` and `sys_task_status`.
    TaskControl,
    /// Authority to call `sys_lease_create`, `sys_lease_revoke`, and
    /// `sys_lease_inspect`.
    LeaseAdmin,
    // ── M7.1 additions (RFC 016, RFC 017) ───────────────────────────────────
    /// A bounded MMIO physical region.  The `object_id` indexes `MmioRegionTable`.
    /// Holder may call `sys_mmio_map(cap, offset, size)`.
    MmioRegion,
    /// A per-task DMA physical region.  The `object_id` indexes `DmaRegionTable`.
    DmaAlloc,
}

/// Permission bits attached to a capability.
///
/// Invariant CAP-A: `child.rights ⊆ parent.rights` must hold after any
/// `cap_copy`, `cap_mint`, or delegation operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct CapRights(pub u32);

impl CapRights {
    pub const SEND:    Self = CapRights(1 << 0);
    pub const RECV:    Self = CapRights(1 << 1);
    pub const CALL:    Self = CapRights(1 << 2);
    pub const GRANT:   Self = CapRights(1 << 3);
    pub const MAP_R:   Self = CapRights(1 << 4);
    pub const MAP_W:   Self = CapRights(1 << 5);
    pub const MAP_X:   Self = CapRights(1 << 6);
    pub const INSPECT: Self = CapRights(1 << 7);

    /// All rights granted.
    pub const ALL: Self = CapRights(0xFF);
    /// No rights.
    pub const NONE: Self = CapRights(0);

    pub fn contains(self, other: CapRights) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns `true` if `self` rights are a subset of `parent` rights.
    ///
    /// Enforces invariant CAP-A.
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
