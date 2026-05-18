//! Fjell OS — stable kernel / user-space ABI.
//!
//! Compiles in both `no_std` (kernel) and `std` (user-space tools) environments.

#![no_std]

pub mod boot;
pub mod error;
pub mod lease;
pub mod service;
pub mod syscall;
pub mod task;
