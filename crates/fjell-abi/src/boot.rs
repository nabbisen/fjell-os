//! Boot-time information passed from kernel to `fjell-init`.
//!
//! The kernel writes a `BootInfo` record into init's address space before
//! starting it, then passes its virtual address in register `a1` at entry.

/// Kernel-to-init boot information.
///
/// Passed as a pointer in `a1` at `fjell-init` entry.  All fields are
/// ABI-stable; new fields are appended and the `version` field bumped.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct BootInfo {
    /// Struct version — currently 1.
    pub version: u32,
    /// Number of bootstrap capability slots pre-installed in init's CSpace.
    pub bootstrap_cap_count: u16,
    /// CSpace slot index of the `TaskCreate` bootstrap capability.
    pub cap_task_create: u16,
    /// CSpace slot index of the `CapDerive` (bootstrap-scope) capability.
    pub cap_derive: u16,
    /// CSpace slot index of the kernel audit-drain capability.
    pub cap_audit_drain: u16,
    /// CSpace slot index of the bootstrap IPC endpoint (for service control).
    pub cap_bootstrap_ep: u16,
    /// Physical RAM size in bytes (informational).
    pub ram_bytes: u64,
}

impl BootInfo {
    /// Bootstrap capability CSpace indices used by `fjell-init`.
    pub const SLOT_TASK_CREATE:  u16 = 0;
    pub const SLOT_CAP_DERIVE:   u16 = 1;
    pub const SLOT_AUDIT_DRAIN:  u16 = 2;
    pub const SLOT_BOOTSTRAP_EP: u16 = 3;
}
