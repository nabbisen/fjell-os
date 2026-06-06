//! # `fjell-sdk` — Curated developer surface for Fjell OS service authors
//!
//! Implements RFC v0.9-001. The SDK re-exports a **stable subset** of
//! Fjell's user-space service surface with explicit per-export stability
//! tiers. External service developers should depend on this crate only,
//! not the underlying workspace crates directly.
//!
//! ## Stability tiers
//!
//! Each re-export carries one of three tiers (see [`tier`] module):
//!
//! - [`tier::Stable`] — covered by semver guarantees for the v0.x.y
//!   minor-release range. Breaking change requires a major-version
//!   bump and at least one deprecation cycle.
//! - [`tier::Provisional`] — semantically frozen but signature may shift
//!   on a v0.(x+1).0 boundary. Use is permitted; expect minor churn.
//! - [`tier::Deprecated`] — superseded; will be removed in a future
//!   version. Migration path is documented on each item.
//!
//! ## Prelude
//!
//! The [`prelude`] module re-exports the 90% subset most services need
//! in one `use fjell_sdk::prelude::*;` line.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// Stability-tier markers used to annotate the SDK surface.
///
/// These are zero-sized types whose only purpose is documentation and
/// (in future) automated semver enforcement. The CI semver-gate
/// inspects which tier a removed export carried before refusing a PR.
pub mod tier {
    /// Stable: covered by semver. Removal or signature change requires
    /// a major-version bump and a deprecation cycle.
    pub struct Stable;
    /// Provisional: usable; signature may shift on minor boundaries.
    pub struct Provisional;
    /// Deprecated: scheduled for removal. See item docs for migration.
    pub struct Deprecated;
}

/// Curated prelude — the 90% subset for service authors.
///
/// ```
/// use fjell_sdk::prelude::*;
/// ```
pub mod prelude {
    // Capability surface (Stable)
    pub use crate::cap::{CapHandle, CapKind, CapRights, CapState};
    // Syscall ABI (Stable) — error type and core syscall numbers
    pub use crate::abi::{SysError, SyscallNumber};
    // Service IPC tags (Stable) — the v0_7 module is the curated tag set
    pub use crate::service_api::v0_7;
    // Semantic emission (Stable) — typed catalog
    pub use crate::semantic::v1::{catalog_len, lookup_tag};
    // Audit (Provisional) — kind labels
    pub use crate::audit::AuditKind;
}

// ── Stable re-exports ────────────────────────────────────────────────────────

/// **Stable.** Syscall ABI types: error codes, syscall numbers, capability
/// kinds. Mirrors `fjell-abi`'s public surface.
pub mod abi {
    pub use fjell_abi::error::SysError;
    pub use fjell_abi::syscall::SyscallNumber;
    pub use fjell_abi::service::{ImageId, ServiceId, TaskLifecycle};
    pub use fjell_abi::lease::{LeaseId, LeaseEpoch};
}

/// **Stable.** Capability types: handle, kind, rights, state.
///
/// Internal types like `CapTable`, `CSpace`, and `Capability` itself are
/// kernel-side and intentionally not re-exported.
pub mod cap {
    pub use fjell_cap::{CapHandle, CapKind, CapRights, CapState, ObjectScope};
}

/// **Stable.** Synchronous IPC primitives. See [`crate::syscall`] for the
/// user-side wrappers.
pub mod ipc {
    pub use fjell_ipc::{IPC_WORDS, MessageTag};
}

/// **Stable.** User-side syscall wrappers (`sys_*` functions).
pub mod syscall {
    pub use fjell_syscall::{
        sys_cap_copy, sys_cap_drop, sys_cap_mint,
        sys_ipc_call, sys_ipc_recv, sys_ipc_recv_msg,
        sys_ipc_reply, sys_ipc_try_recv, sys_ipc_try_send,
        sys_yield,
    };
}

/// **Stable.** Service-IPC protocol tag namespaces.
pub mod service_api {
    pub use fjell_service_api::v0_7;
    pub use fjell_service_api::tags;
}

// ── Semantic ─────────────────────────────────────────────────────────────────

/// **Stable.** Semantic intent emission via the v1 catalog.
pub mod semantic {
    /// The frozen v1 catalog (RFC v0.5-004).
    pub mod v1 {
        pub use fjell_semantic_v1::{
            catalog_len, lookup_tag, CATALOG_V1, CATALOG_V1_VERSION,
            CatalogVersion, IntentEntry,
        };
        pub use fjell_semantic_v1::catalog::{CatalogOwner, CatalogRangeOwner};
    }
    /// **Provisional.** Untyped semantic format primitives. Prefer the
    /// typed [`crate::sdk_emit`] API.
    pub use fjell_semantic_format::{
        EventKind, IntentNode, StateKind, StateNode,
    };
}

// ── Audit ────────────────────────────────────────────────────────────────────

/// **Provisional.** Audit record format. Kernel-side ring layout may
/// shift between v0.9 minor releases as RFC 041 is hardened.
pub mod audit {
    pub use fjell_audit_format::{AuditKind, AuditRecordBin};
}

// ── Typed emitter (RFC v0.9-003 foundation) ──────────────────────────────────

/// **Provisional.** Typed semantic emitter helpers. The full code-generated
/// per-tag structs from RFC v0.9-003 land in v0.9.1; this module establishes
/// the API shape so service authors can opt in to compile-time validation
/// today.
pub mod sdk_emit {
    use crate::semantic::v1::{IntentEntry, lookup_tag};

    /// Look up a catalog entry by tag. Returns `None` if the tag is not
    /// in v1. Service authors building a typed wrapper around the
    /// catalog can use this to assert (at construction time) that a
    /// hard-coded tag is valid.
    pub fn entry_for(tag: u16) -> Option<&'static IntentEntry> {
        lookup_tag(tag)
    }

    /// Returns `true` if the tag is known to v1.
    pub fn is_known_tag(tag: u16) -> bool {
        lookup_tag(tag).is_some()
    }
}

// ── SDK version (Stable) ─────────────────────────────────────────────────────

/// SDK version string. Bumped per workspace minor release.
pub const SDK_VERSION: &str = env!("CARGO_PKG_VERSION");

/// SDK API revision integer. Increments only on intentional breaking
/// surface changes. Allows runtime compatibility probes; the bundle
/// builder (RFC v0.9-004) embeds this value to refuse mismatched
/// bundles.
pub const SDK_API_REV: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prelude_compiles() {
        // The prelude must be usable in a single `use` line.
        // This test verifies that import alone (the prelude module
        // exists and re-exports compile).
        use crate::prelude::*;
        let _: Option<SysError> = None;
        let _: Option<CapKind> = None;
    }

    #[test]
    fn sdk_api_rev_is_positive() {
        assert!(SDK_API_REV >= 1);
    }

    #[test]
    fn sdk_emit_known_tag() {
        // Catalog v1 has 20 entries; tag 0x0101 is one of them
        // (per RFC v0.5-004 and v0.7.5-001 ownership table).
        let entry = sdk_emit::entry_for(0x0101);
        assert!(entry.is_some(), "tag 0x0101 should be in v1 catalog");
    }

    #[test]
    fn sdk_emit_unknown_tag() {
        // Tag 0xFFFF is reserved and never assigned in v1.
        assert!(!sdk_emit::is_known_tag(0xFFFF));
    }

    #[test]
    fn tier_markers_are_zero_sized() {
        use core::mem::size_of;
        assert_eq!(size_of::<tier::Stable>(), 0);
        assert_eq!(size_of::<tier::Provisional>(), 0);
        assert_eq!(size_of::<tier::Deprecated>(), 0);
    }
}
