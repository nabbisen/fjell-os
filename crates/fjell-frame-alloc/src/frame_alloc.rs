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
    /// `start_pa` is inclusive; `end_pa` is exclusive. Frames outside the managed
    /// range are ignored.
    ///
    /// **All or nothing.** If *any* managed frame in the range is already in use --
    /// whatever its owner, the same owner included -- this returns
    /// `AlreadyReserved` and the allocator is left **exactly as it was**: no frame
    /// marked, no owner written, `free_count` and the allocation hint unchanged. The
    /// whole range is checked before anything is mutated. A caller may therefore read
    /// `Err` as "nothing happened".
    ///
    /// There is deliberately no same-owner tolerance. An earlier version of this
    /// comment said a frame already reserved with the same owner did not fail; it did
    /// (the check never read the owner), no caller needed the tolerance, and
    /// behaviour added so that code matches a comment is the wrong direction of
    /// travel (E-103). If a caller ever needs idempotent re-reservation it can be
    /// added then, with a test and a reason.
    ///
    /// Atomic against *failure* within the call, not against concurrency: this
    /// allocator is single-hart and takes no lock.
    pub fn reserve_range(
        &mut self,
        start_pa: usize,
        end_pa: usize,
        owner: FrameOwner,
    ) -> Result<(), MmError> {
        let first_pfn = ((start_pa) >> 12) as u64;
        let last_pfn = ((end_pa + 0xFFF) >> 12) as u64;

        // Pass 1: validate. Nothing is mutated until every frame has been looked at.
        for pfn in first_pfn..last_pfn {
            if let Some(off) = self.pfn_to_offset(pfn) {
                if self.is_used(off) {
                    return Err(MmError::AlreadyReserved);
                }
            }
            // Frames outside our managed range are silently ignored.
        }

        // Pass 2: apply. Every managed frame in the range is free, so this cannot fail.
        for pfn in first_pfn..last_pfn {
            if let Some(off) = self.pfn_to_offset(pfn) {
                self.set_used(off);
                if let Some(ref mut owners) = self.owner {
                    owners[off as usize] = owner;
                }
            }
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

    // ── RFC-0.41-004 (closes E-103): a failed reservation is a no-op ───────────
    //
    // `reserve_range` used to return `AlreadyReserved` at the first used frame having
    // ALREADY marked every frame before it used and written the owner map: a failed call
    // consumed the head of its range and relabelled it (E-103 divergence 2), while the
    // frames it had not reached stayed free (divergence 1), so a caller that read `Err` as
    // "nothing happened" was wrong in both directions. The ruled direction is
    // all-or-nothing: validate the whole range, then apply, so `Err` leaves the allocator
    // bit-for-bit as it was. The tests below pin that, and that any used frame fails
    // whatever its owner (divergence 3: the doc promised a same-owner tolerance the code
    // never had; the doc was wrong, and the tolerance is deliberately not built).

    /// An allocator WITH an owner map, so `owner_of` can be asserted.
    fn ram_owned(frames: u64) -> FrameAllocator<'static> {
        let words = ((frames + 63) / 64) as usize;
        let bitmap: &'static mut [u64] = Box::leak(vec![0u64; words].into_boxed_slice());
        let owners: &'static mut [FrameOwner] =
            Box::leak(vec![FrameOwner::Free; frames as usize].into_boxed_slice());
        FrameAllocator::new(0, frames, bitmap, Some(owners))
    }

    /// Everything a reservation can touch, copied out: the bitmap, the allocation hint,
    /// the owner map and the free count. "Bit-for-bit untouched" is equality of this.
    fn snapshot(fa: &FrameAllocator<'_>) -> (Vec<u64>, u64, Option<Vec<FrameOwner>>, u64) {
        (
            fa.bitmap.to_vec(),
            fa.next_hint,
            fa.owner.as_deref().map(|o| o.to_vec()),
            fa.free_count(),
        )
    }

    /// Divergence 2, in the architect's words: reserve `[4,8)` as `KernelText`, then
    /// `[0,8)` as `KernelStack`. The call must fail AND leave `free_count` and the owner
    /// of every frame exactly as it was -- both halves, because the probe found both
    /// (`free_count` dropped by 4 and frames 0-3 reported `KernelStack`).
    #[test]
    fn a_failed_reservation_neither_consumes_nor_relabels_the_head_of_its_range() {
        let mut fa = ram_owned(16);
        fa.reserve_range(pa(4), pa(8), FrameOwner::KernelText)
            .unwrap();
        let free_before = fa.free_count();

        let r = fa.reserve_range(pa(0), pa(8), FrameOwner::KernelStack);

        assert_eq!(r, Err(MmError::AlreadyReserved));
        assert_eq!(
            fa.free_count(),
            free_before,
            "a failed call consumed frames"
        );
        for f in 0..4 {
            let frame = PhysFrame::from_pfn(f).unwrap();
            assert_eq!(
                fa.owner_of(frame),
                Some(FrameOwner::Free),
                "frame {f}: the head of a failed range was relabelled"
            );
        }
        for f in 4..8 {
            let frame = PhysFrame::from_pfn(f).unwrap();
            assert_eq!(
                fa.owner_of(frame),
                Some(FrameOwner::KernelText),
                "frame {f}: an existing owner was overwritten"
            );
        }
    }

    /// Divergence 1, restated under the ruled design. The old test asserted that after a
    /// failed `[2,6)` over `[0,4)` the tail `[4,6)` was NOT free -- i.e. that a failed call
    /// should reserve what it could reach. All-or-nothing is the opposite contract: `Err`
    /// means nothing was reserved, so the tail is free **because the caller was told the
    /// reservation did not happen**, and the allocator is exactly as it was.
    #[test]
    fn the_frames_a_failed_reservation_did_not_reach_are_exactly_as_they_were() {
        let mut fa = ram_owned(16);
        fa.reserve_range(pa(0), pa(4), FrameOwner::KernelText)
            .unwrap();
        let before = snapshot(&fa);

        let r = fa.reserve_range(pa(2), pa(6), FrameOwner::KernelStack);

        assert_eq!(r, Err(MmError::AlreadyReserved));
        assert_eq!(snapshot(&fa), before);
        assert_eq!(
            fa.first_free_in(pa(4), pa(6)),
            Some(pa(4)),
            "the tail is untouched, so still free"
        );
    }

    /// The general statement, over every place the conflict can sit in the range (first,
    /// middle, last frame) and with and without an owner map: a failed call is a no-op.
    /// The range asked for is `[8, 16)`; an allocation first takes frame 0, so the hint is
    /// not at its initial value and the allocation sits outside the range.
    #[test]
    fn a_failed_reservation_leaves_the_allocator_bit_for_bit_untouched() {
        for conflict_at in [8usize, 11, 15] {
            for with_owners in [false, true] {
                let mut fa = if with_owners { ram_owned(16) } else { ram(16) };
                let taken = fa.alloc_frame(FrameOwner::KernelMeta).unwrap();
                assert!(taken.pfn < 8, "the allocation must sit outside the range");
                // One frame already in use, somewhere inside the range [8, 16) to be asked for.
                fa.reserve_range(pa(conflict_at), pa(conflict_at + 1), FrameOwner::KernelData)
                    .unwrap();
                let before = snapshot(&fa);

                let r = fa.reserve_range(pa(8), pa(16), FrameOwner::KernelStack);

                assert_eq!(r, Err(MmError::AlreadyReserved));
                assert_eq!(
                    snapshot(&fa),
                    before,
                    "conflict at frame {conflict_at}, owner map {with_owners}: a failed call changed state"
                );
            }
        }
    }

    /// Divergence 3. The doc used to say a frame already reserved **with the same owner**
    /// does not fail. It does fail, the tolerance is deliberately not built (no caller
    /// needs it; behaviour added so that code matches a comment is the wrong direction),
    /// and the doc now says so. Pinned here so "make the code match the old comment" cannot
    /// come back unnoticed: a same-owner overlap fails, and fails as a no-op.
    #[test]
    fn a_same_owner_overlapping_reservation_fails_and_changes_nothing() {
        let mut fa = ram_owned(16);
        fa.reserve_range(pa(4), pa(6), FrameOwner::KernelStack)
            .unwrap();
        let before = snapshot(&fa);

        // Same owner, overlapping: frames 2 and 3 are free, frame 4 is the conflict.
        let r = fa.reserve_range(pa(2), pa(6), FrameOwner::KernelStack);

        assert_eq!(
            r,
            Err(MmError::AlreadyReserved),
            "the same-owner tolerance exists again"
        );
        assert_eq!(snapshot(&fa), before);
    }

    /// What the rewrite must NOT change: a successful reservation marks every frame and
    /// writes the owner, and frames outside the managed range are still ignored.
    #[test]
    fn a_successful_reservation_still_marks_every_frame_and_ignores_unmanaged_ones() {
        let mut fa = ram_owned(8);
        // [6, 12): frames 6 and 7 are managed, 8..12 are not.
        fa.reserve_range(pa(6), pa(12), FrameOwner::Mmio).unwrap();
        assert_eq!(fa.free_count(), 6);
        for f in 6..8 {
            assert_eq!(
                fa.owner_of(PhysFrame::from_pfn(f).unwrap()),
                Some(FrameOwner::Mmio)
            );
        }
        assert_eq!(
            fa.owner_of(PhysFrame::from_pfn(5).unwrap()),
            Some(FrameOwner::Free)
        );
    }
}
