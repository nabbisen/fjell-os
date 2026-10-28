//! Task identifier — re-exported from `fjell-abi` to avoid duplication.
//!
//! `FrameOwner` in `fjell-frame-alloc` (re-exported as `mm::frame_alloc`) uses `fjell_abi::task::TaskId` directly,
//! so the kernel's `TaskId` must be the same type.

pub use fjell_abi::task::TaskId;
