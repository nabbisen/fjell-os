//! Bitmap frame allocator for 4 KiB physical frames.
//!
//! Uses a next-fit scan over a bitmap where each bit represents one frame.
//! An optional `FrameOwner` array provides debug-level ownership tracking.
//!
//! Items unused in M2 (free_frame, owner_of, set_free) are kept for M3+.
#![allow(dead_code)]

pub use super::address::PhysFrame;
use super::error::MmError;
use fjell_abi::task::TaskId;

/// Physical frame size in bytes.
pub const FRAME_SIZE: usize = 4096;

/// Ownership tag for each physical frame (debug / audit use).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameOwner {
    Free,
    ReservedBoot,
    KernelText,
    KernelRodata,
    KernelData,
    KernelBss,
    KernelStack,
    KernelPageTable,
    KernelMeta,
    Dtb,
    Mmio,
    UserText {
        task: TaskId,
    },
    UserData {
        task: TaskId,
    },
    UserStack {
        task: TaskId,
    },
    /// A page `sys_dma_alloc` gave `task` for device I/O (RFC-0.39-003 follow-up).
    /// It used to be recorded as `UserStack`, the same label a task's *actual
    /// stack* frame carries, so an ownership check could not tell the two apart
    /// and the owner map -- added to make this defect class diagnosable --
    /// would have told the next investigation that DMA frames were stacks.
    Dma {
        task: TaskId,
    },
}

/// Bitmap frame allocator.
///
/// The `bitmap` and optional `owner` slices must be backed by memory
/// allocated from `BootAllocator` before `FrameAllocator` is constructed.
pub struct FrameAllocator<'a> {
    base_pfn: u64,
    frame_count: u64,
    bitmap: &'a mut [u64],
    next_hint: u64,
    owner: Option<&'a mut [FrameOwner]>,
}

impl<'a> FrameAllocator<'a> {
    /// Construct a new allocator.
    ///
    /// `base_pfn` is the PFN of the first frame managed.
    /// `bitmap` must be zeroed (all frames initially free).
    pub fn new(
        base_pfn: u64,
        frame_count: u64,
        bitmap: &'a mut [u64],
        owner: Option<&'a mut [FrameOwner]>,
    ) -> Self {
        debug_assert!(bitmap.len() as u64 >= (frame_count + 63) / 64);
        debug_assert!(
            owner
                .as_deref()
                .is_none_or(|o| o.len() as u64 >= frame_count),
            "the owner array must cover every managed frame"
        );
        FrameAllocator {
            base_pfn,
            frame_count,
            bitmap,
            next_hint: 0,
            owner,
        }
    }

    // ── bitmap helpers ────────────────────────────────────────────────────────

    fn is_used(&self, pfn_offset: u64) -> bool {
        let word = (pfn_offset / 64) as usize;
        let bit = pfn_offset % 64;
        self.bitmap[word] & (1 << bit) != 0
    }

    fn set_used(&mut self, pfn_offset: u64) {
        let word = (pfn_offset / 64) as usize;
        let bit = pfn_offset % 64;
        self.bitmap[word] |= 1 << bit;
    }

    fn set_free(&mut self, pfn_offset: u64) {
        let word = (pfn_offset / 64) as usize;
        let bit = pfn_offset % 64;
        self.bitmap[word] &= !(1 << bit);
    }

    fn pfn_to_offset(&self, pfn: u64) -> Option<u64> {
        pfn.checked_sub(self.base_pfn)
            .filter(|&o| o < self.frame_count)
    }

    // ── public API ────────────────────────────────────────────────────────────

    /// Mark a physical address range as reserved with the given owner.
    ///
    /// `start_pa` is inclusive; `end_pa` is exclusive.
    /// Does not fail if a frame is already reserved with the same owner,
    /// but returns `AlreadyReserved` for conflicting owners.
    pub fn reserve_range(
        &mut self,
        start_pa: usize,
        end_pa: usize,
        owner: FrameOwner,
    ) -> Result<(), MmError> {
        let first_pfn = ((start_pa) >> 12) as u64;
        let last_pfn = ((end_pa + 0xFFF) >> 12) as u64;

        for pfn in first_pfn..last_pfn {
            if let Some(off) = self.pfn_to_offset(pfn) {
                if self.is_used(off) {
                    return Err(MmError::AlreadyReserved);
                }
                self.set_used(off);
                if let Some(ref mut owners) = self.owner {
                    owners[off as usize] = owner;
                }
            }
            // Frames outside our managed range are silently ignored.
        }
        Ok(())
    }

    /// Allocate one free frame using next-fit scan.
    pub fn alloc_frame(&mut self, owner: FrameOwner) -> Result<PhysFrame, MmError> {
        let start = self.next_hint;
        let count = self.frame_count;

        for i in 0..count {
            let off = (start + i) % count;
            if !self.is_used(off) {
                self.set_used(off);
                if let Some(ref mut owners) = self.owner {
                    owners[off as usize] = owner;
                }
                self.next_hint = (off + 1) % count;
                return Ok(PhysFrame {
                    pfn: self.base_pfn + off,
                });
            }
        }
        Err(MmError::OutOfMemory)
    }

    /// Free a previously-allocated frame.
    ///
    /// Returns `DoubleFree` if the frame is already free (invariant MM-PHY-005).
    pub fn free_frame(&mut self, frame: PhysFrame) -> Result<(), MmError> {
        let off = self.pfn_to_offset(frame.pfn).ok_or(MmError::InvalidRange)?;
        if !self.is_used(off) {
            return Err(MmError::DoubleFree);
        }
        self.set_free(off);
        if let Some(ref mut owners) = self.owner {
            owners[off as usize] = FrameOwner::Free;
        }
        Ok(())
    }

    /// Return the owner of a frame, if the owner array is present.
    pub fn owner_of(&self, frame: PhysFrame) -> Option<FrameOwner> {
        let off = self.pfn_to_offset(frame.pfn)? as usize;
        self.owner.as_deref().map(|o| o[off])
    }

    /// The lowest physical address in `[start_pa, end_pa)` whose frame is **free**,
    /// or `None` if every managed frame in the range is in use.
    ///
    /// RFC-0.39-003: the kernel asserts this is `None` over its own image and
    /// stack at boot. A free frame there is handed to a task, and the kernel then
    /// writes its stack through that task's memory (E-097).
    pub fn first_free_in(&self, start_pa: usize, end_pa: usize) -> Option<usize> {
        let first_pfn = (start_pa >> 12) as u64;
        let last_pfn = ((end_pa + 0xFFF) >> 12) as u64;
        (first_pfn..last_pfn)
            .filter_map(|pfn| self.pfn_to_offset(pfn).map(|off| (pfn, off)))
            .find(|&(_, off)| !self.is_used(off))
            .map(|(pfn, _)| (pfn << 12) as usize)
    }

    /// Number of free frames remaining.
    pub fn free_count(&self) -> u64 {
        let used: u64 = self.bitmap.iter().map(|w| w.count_ones() as u64).sum();
        self.frame_count - used
    }
}

// ── host-side unit tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::{boxed::Box, vec, vec::Vec};

    fn make_allocator(frames: u64) -> (Vec<u64>, FrameAllocator<'static>) {
        // This leaks memory but is fine for unit tests.
        let bitmap_words = ((frames + 63) / 64) as usize;
        let bitmap: &'static mut [u64] = Box::leak(vec![0u64; bitmap_words].into_boxed_slice());
        let alloc = FrameAllocator::new(0, frames, bitmap, None);
        (vec![], alloc)
    }

    #[test]
    fn alloc_and_free() {
        let bitmap_words = 4usize;
        let bitmap: &'static mut [u64] = Box::leak(vec![0u64; bitmap_words].into_boxed_slice());
        let mut fa = FrameAllocator::new(0, 256, bitmap, None);

        let f0 = fa.alloc_frame(FrameOwner::KernelData).unwrap();
        let f1 = fa.alloc_frame(FrameOwner::KernelData).unwrap();
        assert_ne!(f0.pfn, f1.pfn);
        assert_eq!(fa.free_count(), 254);

        fa.free_frame(f0).unwrap();
        assert_eq!(fa.free_count(), 255);
    }

    #[test]
    fn double_free_is_detected() {
        let bitmap: &'static mut [u64] = Box::leak(vec![0u64; 1].into_boxed_slice());
        let mut fa = FrameAllocator::new(0, 64, bitmap, None);
        let f = fa.alloc_frame(FrameOwner::KernelData).unwrap();
        fa.free_frame(f).unwrap();
        assert_eq!(fa.free_frame(f), Err(MmError::DoubleFree));
    }

    #[test]
    fn first_free_in_reports_the_lowest_gap() {
        let bitmap: &'static mut [u64] = Box::leak(vec![0u64; 1].into_boxed_slice());
        let mut fa = FrameAllocator::new(0, 64, bitmap, None);
        // Frames 0-3 and 6-9 reserved: the gap at frames 4-5 is the answer.
        fa.reserve_range(0x0000, 0x4000, FrameOwner::KernelText)
            .unwrap();
        fa.reserve_range(0x6000, 0xA000, FrameOwner::KernelStack)
            .unwrap();
        assert_eq!(fa.first_free_in(0x0000, 0xA000), Some(0x4000));
        // A range that is fully reserved has none.
        assert_eq!(fa.first_free_in(0x0000, 0x4000), None);
        assert_eq!(fa.first_free_in(0x6000, 0xA000), None);
        // Closing the gap leaves nothing free across the whole span.
        fa.reserve_range(0x4000, 0x6000, FrameOwner::KernelData)
            .unwrap();
        assert_eq!(fa.first_free_in(0x0000, 0xA000), None);
        // Frames outside the managed range are not reported.
        assert_eq!(fa.first_free_in(0x100000, 0x200000), None);
    }

    #[test]
    fn the_owner_map_names_who_holds_a_frame() {
        let bitmap: &'static mut [u64] = Box::leak(vec![0u64; 1].into_boxed_slice());
        let owners: &'static mut [FrameOwner] =
            Box::leak(vec![FrameOwner::Free; 64].into_boxed_slice());
        let mut fa = FrameAllocator::new(0, 64, bitmap, Some(owners));
        let task = TaskId::new(7, 0);
        let f = fa.alloc_frame(FrameOwner::UserStack { task }).unwrap();
        assert_eq!(fa.owner_of(f), Some(FrameOwner::UserStack { task }));
        fa.free_frame(f).unwrap();
        assert_eq!(fa.owner_of(f), Some(FrameOwner::Free));
    }

    #[test]
    fn a_dma_frame_is_not_labelled_a_stack() {
        // The owner map exists to say who holds a frame. A DMA page and a task's
        // own stack page for the same task must be different answers, or a check
        // that "this frame is the task's DMA frame" confirms nothing.
        let bitmap: &'static mut [u64] = Box::leak(vec![0u64; 1].into_boxed_slice());
        let owners: &'static mut [FrameOwner] =
            Box::leak(vec![FrameOwner::Free; 64].into_boxed_slice());
        let mut fa = FrameAllocator::new(0, 64, bitmap, Some(owners));
        let task = TaskId::new(7, 0);
        let dma = fa.alloc_frame(FrameOwner::Dma { task }).unwrap();
        let stack = fa.alloc_frame(FrameOwner::UserStack { task }).unwrap();
        assert_eq!(fa.owner_of(dma), Some(FrameOwner::Dma { task }));
        assert_eq!(fa.owner_of(stack), Some(FrameOwner::UserStack { task }));
        assert_ne!(fa.owner_of(dma), fa.owner_of(stack));
    }

    #[test]
    fn reserve_range_excludes_frames() {
        let bitmap: &'static mut [u64] = Box::leak(vec![0u64; 1].into_boxed_slice());
        let mut fa = FrameAllocator::new(0, 64, bitmap, None);
        // Reserve frames 0–3 (PFN 0..4, PA 0x0000..0x4000).
        fa.reserve_range(0x0000, 0x4000, FrameOwner::KernelText)
            .unwrap();
        assert_eq!(fa.free_count(), 60);
        // The first allocation should skip past the reserved frames.
        let f = fa.alloc_frame(FrameOwner::KernelData).unwrap();
        assert!(f.pfn >= 4);
    }

    // ── RFC-0.40-003 R5: the tests E-097 would have failed ────────────────────
    //
    // E-097 was a reservation defect: a 4 MiB kernel stack of which the allocator
    // reserved only the first half, so the second half was free, was handed to
    // tasks, and the kernel stack overwrote them. The six tests above did not
    // speak to it. These assert the property the boot invariant
    // (`first_free_in(RAM_BASE, __stack_top)` is `None`) enforces, on the layout
    // that failed, in miniature: frames are numbered, image = 0..8, stack = 8..40,
    // and the boot window the kernel reserved was 16 frames starting at the
    // stack's bottom.

    const PAGE: usize = 0x1000;
    fn pa(frame: usize) -> usize {
        frame * PAGE
    }
    fn ram(frames: u64) -> FrameAllocator<'static> {
        let words = ((frames + 63) / 64) as usize;
        let bitmap: &'static mut [u64] = Box::leak(vec![0u64; words].into_boxed_slice());
        FrameAllocator::new(0, frames, bitmap, None)
    }

    #[test]
    fn e097_a_stack_reserved_by_the_coincidence_of_a_window_leaves_frames_free() {
        // THE DEFECT, built deliberately: image reserved, then a "boot window" of
        // 16 frames that begins where the stack begins -- so the stack's first 16
        // frames look reserved and its last 16 do not.
        let mut fa = ram(128);
        fa.reserve_range(pa(0), pa(8), FrameOwner::KernelText)
            .unwrap();
        fa.reserve_range(pa(8), pa(24), FrameOwner::ReservedBoot)
            .unwrap();
        // This is what the boot invariant looks for, and what it finds: the first
        // free frame under the stack top is the stack's own 17th frame. If this
        // returned `None` the test would not be demonstrating the failure.
        assert_eq!(fa.first_free_in(pa(0), pa(40)), Some(pa(24)));
        // ...and the allocator would hand it out, which is the damage.
        let mut given = std::vec::Vec::new();
        while let Ok(f) = fa.alloc_frame(FrameOwner::Free) {
            given.push(f.pa());
            if given.len() > 100 {
                break;
            }
        }
        assert!(
            given.contains(&pa(24)) && given.contains(&pa(39)),
            "frames inside the stack range are allocatable"
        );
    }

    #[test]
    fn e097_the_whole_stack_reserved_leaves_nothing_free_inside_it() {
        let mut fa = ram(128);
        fa.reserve_range(pa(0), pa(8), FrameOwner::KernelText)
            .unwrap();
        fa.reserve_range(pa(8), pa(40), FrameOwner::KernelStack)
            .unwrap();
        fa.reserve_range(pa(40), pa(56), FrameOwner::ReservedBoot)
            .unwrap();
        assert_eq!(fa.first_free_in(pa(0), pa(40)), None);
        // And nothing in that range is ever allocated.
        while let Ok(f) = fa.alloc_frame(FrameOwner::Free) {
            assert!(
                f.pa() >= pa(56),
                "allocated {:#x} inside a reserved range",
                f.pa()
            );
        }
    }

    #[test]
    fn a_range_reserved_across_two_calls_leaves_nothing_free_inside_it() {
        // Adjacent halves, in either order: the union is reserved.
        for (first, second) in [((8, 24), (24, 40)), ((24, 40), (8, 24))] {
            let mut fa = ram(128);
            fa.reserve_range(pa(first.0), pa(first.1), FrameOwner::KernelStack)
                .unwrap();
            fa.reserve_range(pa(second.0), pa(second.1), FrameOwner::KernelStack)
                .unwrap();
            assert_eq!(
                fa.first_free_in(pa(8), pa(40)),
                None,
                "order {first:?} then {second:?}"
            );
        }
    }

    #[test]
    fn a_range_reserved_after_allocation_has_begun_leaves_nothing_free_inside_it() {
        let mut fa = ram(128);
        // Allocation starts first (next-fit from frame 0), then a range above it
        // is reserved: the next-fit hint must not matter.
        let early: std::vec::Vec<usize> = (0..5)
            .map(|_| fa.alloc_frame(FrameOwner::KernelData).unwrap().pa())
            .collect();
        assert_eq!(early, [pa(0), pa(1), pa(2), pa(3), pa(4)]);
        fa.reserve_range(pa(8), pa(40), FrameOwner::KernelStack)
            .unwrap();
        assert_eq!(fa.first_free_in(pa(8), pa(40)), None);
        // Later allocations fill the gap below the range and then skip over it.
        while let Ok(f) = fa.alloc_frame(FrameOwner::Free) {
            assert!(
                f.pa() < pa(8) || f.pa() >= pa(40),
                "allocated {:#x} inside a range reserved after allocation began",
                f.pa()
            );
        }
    }

    #[test]
    fn first_free_in_agrees_with_what_alloc_frame_can_return() {
        // The invariant is only worth anything if it speaks about the same frames
        // the allocator hands out: every frame `alloc_frame` yields lies where
        // `first_free_in` said something was free, and nowhere else.
        let mut fa = ram(64);
        fa.reserve_range(pa(0), pa(10), FrameOwner::KernelText)
            .unwrap();
        fa.reserve_range(pa(20), pa(30), FrameOwner::KernelStack)
            .unwrap();
        let mut seen = std::vec::Vec::new();
        while let Ok(f) = fa.alloc_frame(FrameOwner::Free) {
            seen.push(f.pa());
        }
        seen.sort_unstable();
        let want: std::vec::Vec<usize> = (10..20).chain(30..64).map(pa).collect();
        assert_eq!(seen, want);
        assert_eq!(
            fa.first_free_in(pa(0), pa(64)),
            None,
            "everything is now taken"
        );
    }

    // ── What these tests FOUND (RFC-0.40-003: file and escalate, do not fix) ───
    //
    // `reserve_range` stops at the first frame that is already in use and returns
    // `AlreadyReserved`, but the frames it had NOT reached yet stay free, so an
    // overlapping reservation silently leaves the tail of its range unreserved --
    // the E-097 shape, one level down. Its doc comment also says it "does not fail
    // if a frame is already reserved with the same owner"; the code fails on any
    // used frame. No caller in the kernel overlaps today (checked), so nothing is
    // wrong yet. The property below is the one the boot invariant relies on; it is
    // `#[ignore]`d, not deleted and not passing, until E-103 (scheduled 0.41, ruled:
    // make `reserve_range` all-or-nothing) is fixed. It covers divergence 1 of E-103
    // only; the entry names all three.
    #[test]
    #[ignore = "E-103: reserve_range leaves the unreached tail of an overlapping range free; ruled all-or-nothing, scheduled 0.41"]
    fn an_overlapping_reservation_does_not_leave_the_rest_of_its_range_free() {
        let mut fa = ram(64);
        fa.reserve_range(pa(0), pa(4), FrameOwner::KernelText)
            .unwrap();
        let _ = fa.reserve_range(pa(2), pa(6), FrameOwner::KernelStack);
        assert_eq!(fa.first_free_in(pa(0), pa(6)), None);
    }
}
