//! Task-related ABI types shared between kernel and user space.

/// Opaque task identifier visible across the ABI boundary.
///
/// Carries a generation counter to detect stale handles (M3+).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct TaskId {
    pub index: u16,
    pub generation: u16,
}

impl TaskId {
    #[inline]
    pub const fn new(index: u16, generation: u16) -> Self {
        TaskId { index, generation }
    }
}
