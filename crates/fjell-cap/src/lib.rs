//! Capability model for Fjell OS — pure logic, host-testable (RFC 031 / RFC 032).
//!
//! This crate is free of arch and platform dependencies so the full capability
//! state machine can be unit-tested on the host before integration into the kernel.
//!
//! # Key types (v0.2.0)
//!
//! - [`CapHandle`]     — generation-tagged slot reference
//! - [`CapRights`]     — 64-bit permission bitmask (extended in RFC 031)
//! - [`CapKind`]       — discriminant for the referenced kernel object
//! - [`CapState`]      — lifecycle state of a capability object
//! - [`ObjectScope`]   — object-level scope restriction (RFC 031 §2.3)
//! - [`CapError`]      — typed enforcement error (RFC 031 §2.7)
//! - [`Capability`]    — capability with rights, scope, lease binding
//! - [`CapSlot`]       — one CSpace slot with generation and state
//! - [`CapSlotState`]  — Empty / Active / Dropped (RFC 032 §2.1)
//! - [`CSpace`]        — per-task fixed-capacity capability table
//!
//! # Enforcement
//!
//! The central enforcement function is [`enforcement::require_cap`].
//! It replaces all `caller_has_cap(kind)` / task-id allowlist / debug-bypass
//! patterns present in v0.1.x (RFC 031).
//!
//! The explicit slot-release function is [`enforcement::cap_drop`] (RFC 032).

#![no_std]
#![allow(dead_code)]

pub use fjell_abi::task::TaskId;

pub mod handle;
pub mod rights;
pub mod slot;
pub mod cspace;
pub mod enforcement;

// Re-exports for ergonomic use
pub use handle::CapHandle;
pub use rights::{CapError, CapKind, CapRights, CapState, ObjectScope};
pub use slot::{AlwaysRevoked, Capability, CapSlot, CapSlotState, LeaseBinding, LeaseChecker, NoLease};
pub use cspace::{CSpace, CSPACE_SLOTS};
pub use enforcement::{cap_drop, require_cap};
