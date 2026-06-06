//! Capability, IPC, and lease property-test harness (RFC v0.6-001).
//!
//! A pure in-process model of Fjell OS cap/IPC/lease semantics for
//! property-based testing with `proptest`.  No kernel calls; operations
//! manipulate a `ModelState` directly.

pub mod model;
pub mod ops;
pub mod properties;
pub mod generators;
