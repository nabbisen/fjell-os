//! Task identifier — re-exported from `fjell-abi` to avoid duplication.
//!
//! `FrameOwner` in `mm/frame_alloc.rs` uses `fjell_abi::task::TaskId` directly,
//! so the kernel's `TaskId` must be the same type.

pub use fjell_abi::task::TaskId;
