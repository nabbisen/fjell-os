//! Kernel capability subsystem.
//!
//! Wraps `fjell-cap` with kernel-internal state (endpoint table, per-task
//! CSpace storage) so that syscall handlers can look up objects by
//! `object_id`.

pub mod table;
pub mod syscall;
