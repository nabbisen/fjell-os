//! The kernel's physical-frame allocator, in a crate a gate can run
//! (RFC-0.40-003, closes E-100).
//!
//! # Why this is a crate and not three files in `fjell-kernel`
//!
//! The allocator's unit tests used to live inside `fjell-kernel`, a `no_std`
//! binary with no host target, and were written against `std` (`Box::leak`,
//! `vec!`). So there was no target that compiled them and no gate that ran them.
//! A reservation defect in this code -- the 4 MiB kernel stack half outside the
//! reserved range, E-097 -- lived thirty-five milestones for exactly that reason.
//! Here `cargo test --workspace --lib` reaches them, which is the whole change.
//!
//! # What moved, and what did not
//!
//! Three files moved unchanged except for their module paths: [`frame_alloc`]
//! (the bitmap allocator), [`address`] (`PhysFrame`, `PhysAddr`, `VirtAddr`) and
//! [`error`] (`MmError`). They reference nothing else in the kernel. The kernel's
//! `mm` module re-exports all three, so the other files that name
//! `crate::mm::frame_alloc::...` are untouched. The page tables, address spaces
//! and the rest of the memory manager stay where they were: they are coupled to
//! the kernel and are not this line.
//!
//! # No `unsafe`
//!
//! A bitmap over a caller-supplied `&mut [u64]` needs none, and this crate
//! forbids it. The kernel's own `unsafe` around the allocator -- the static
//! storage, the identity-mapped frame writes -- stays in the kernel.
#![no_std]
#![forbid(unsafe_code)]

pub mod address;
pub mod error;
pub mod frame_alloc;

pub use address::{PhysAddr, PhysFrame, VirtAddr};
pub use error::MmError;
pub use frame_alloc::{FRAME_SIZE, FrameAllocator, FrameOwner};

// The tests were written against `std` (`Box::leak`, `vec!`), and a test build is
// the one place that is fine.
#[cfg(test)]
extern crate std;
