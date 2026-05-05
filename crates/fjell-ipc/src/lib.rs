//! IPC message and endpoint type definitions for Fjell OS.
//!
//! Pure logic crate — no arch dependencies — so the full IPC state machine
//! can be property-tested on the host before integration into the kernel.
//!
//! Synchronous rendezvous IPC (L4/seL4 style) is the target model.
//! Implemented in M3.

#![no_std]
