//! Capability model for Fjell OS.
//!
//! Pure-logic, host-testable crate.  No arch dependencies.
//! M2: type stubs only.  Full capability table implemented in M3.

#![no_std]

// Re-export TaskId from fjell-abi for capability consumers.
pub use fjell_abi::task::TaskId;
