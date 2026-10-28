//! Memory management subsystem.

// RFC-0.40-003 (E-100): the frame allocator, the address types and `MmError` moved
// to `fjell-frame-alloc`, which the host can build and test. Re-exported under
// their old names so every `crate::mm::frame_alloc::...` in the kernel is unchanged.
pub use fjell_frame_alloc::{address, error, frame_alloc};
pub mod boot_alloc;
pub mod page_table;
pub mod region;
pub mod vspace;

pub mod user_copy;
pub mod user_ptr;
