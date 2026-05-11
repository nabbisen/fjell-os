//! Monotonic bump allocator used only during kernel initialisation.
#![allow(dead_code)]
//!
//! Provides memory for the frame-allocator bitmap, initial page tables,
//! task table, scheduler queues, and audit ring — all of which have known
//! sizes before the frame allocator becomes active.
#![allow(dead_code)]
//!
//! # Invariants
//! - `free` is not provided; allocations are permanent.
//! - The allocator must not be used after `FrameAllocator` is initialised.
//! - All regions allocated here are marked `FrameOwner::ReservedBoot` in the
//!   frame allocator.
#![allow(dead_code)]

use super::error::MmError;

pub struct BootAllocator {
    cur: usize,
    end: usize,
}

impl BootAllocator {
    /// Create a new allocator covering `[start, end)`.
    pub const fn new(start: usize, end: usize) -> Self {
        BootAllocator { cur: start, end }
    }

    /// Allocate `size` bytes with the given power-of-two `align`.
    ///
    /// Returns the physical address of the allocated region.
    pub fn alloc_aligned(&mut self, size: usize, align: usize) -> Result<usize, MmError> {
        debug_assert!(align.is_power_of_two(), "align must be a power of two");
        let aligned = (self.cur + align - 1) & !(align - 1);
        let end = aligned.checked_add(size).ok_or(MmError::InvalidRange)?;
        if end > self.end {
            return Err(MmError::OutOfMemory);
        }
        self.cur = end;
        Ok(aligned)
    }

    /// Return the current watermark (next free address).
    #[inline]
    pub fn watermark(&self) -> usize {
        self.cur
    }
}
