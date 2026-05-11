//! Capability model for Fjell OS — pure logic, host-testable.
//!
//! This crate is free of arch and platform dependencies so that the full
//! capability state machine can be unit-tested and property-tested on the
//! host before integration into the kernel.
//!
//! # Key types
//! - [`CapHandle`]   — opaque generation-tagged slot reference
//! - [`CapRights`]   — permission bitmask
//! - [`CapKind`]     — discriminant for the referenced kernel object
//! - [`Capability`]  — a single capability with its rights, badge, and
//!                     derivation-tree links
//! - [`CapSlot`]     — one slot inside a CSpace
//! - [`CSpace`]      — per-task fixed-capacity capability table

#![no_std]
#![allow(dead_code)]

pub use fjell_abi::task::TaskId;

pub mod handle;
pub mod rights;
pub mod slot;
pub mod cspace;

pub use handle::CapHandle;
pub use rights::{CapKind, CapRights};
pub use slot::{CapSlot, Capability};
pub use cspace::CSpace;
