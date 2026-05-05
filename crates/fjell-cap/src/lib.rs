//! Capability model for Fjell OS.
//!
//! Defines capability types, rights, and state transitions that are shared
//! between the kernel (enforcement) and user-space (inspection/delegation).
//! This crate is host-testable pure logic — no arch or platform dependencies.
//!
//! Implemented progressively:
//! - M2: type stubs and `TaskId` for frame ownership.
//! - M3: `Capability`, `CapRights`, `CapHandle`, derivation tree.

#![no_std]

/// A task identifier.  Carries a generation counter to detect stale handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaskId {
    pub index: u16,
    pub generation: u16,
}
