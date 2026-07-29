//! Build-time derivation of `BoardProfile` from a Device Tree Blob (DTB).
//!
//! RFC v0.5-002: `devmgr` calls `derive_board_profile()` at boot with the
//! DTB bytes handed off by the kernel.  The result is a `BoardProfile` ready
//! for digest verification.
//!
//! This crate contains only a minimal DTB parser sufficient to extract the
//! device list needed by Fjell OS.  It does not attempt to be a general-
//! purpose FDT library.
#![no_std]

pub mod compat;
pub mod derive;
pub mod parser;

pub use compat::{ALLOWED_COMPAT_QEMU_VIRT, CompatString};
pub use derive::{DeriveContext, DeriveError, derive_board_profile};

#[cfg(test)]
mod tests;
