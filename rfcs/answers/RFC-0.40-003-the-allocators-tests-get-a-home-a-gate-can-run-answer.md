# RFC-0.40-003 R1–R7: the allocator's tests get a home a gate can run

**Governing RFC:** [../done/RFC-0.40-003-the-allocators-tests-get-a-home-a-gate-can-run.md](../done/RFC-0.40-003-the-allocators-tests-get-a-home-a-gate-can-run.md)
**Handoff:** [../handoffs/RFC-0.40-003-the-allocators-tests-get-a-home-a-gate-can-run/implementation-handoff.md](../handoffs/RFC-0.40-003-the-allocators-tests-get-a-home-a-gate-can-run/implementation-handoff.md)

Written against base `0555d76`. **E-100 closes in this line** (R6). The line also produced a
finding in the allocator itself (below), which it does not fix.

## R1 — the coupling, by command

`/usr/bin/grep -a -c 'crate::'` over the three files: `frame_alloc.rs` **0**, `address.rs` **0**,
`error.rs` **0**; control, `page_table.rs`: 1. `frame_alloc.rs` imports exactly
`super::address::PhysFrame`, `super::error::MmError` and `fjell_abi::task::TaskId`;
`address.rs` imports `super::error::MmError`. **415 lines** (309 + 80 + 26), as measured. `unsafe`
in the three: **0**. The architect's measurements held; I did not
re-count the kernel files that use the two types, because the re-export meant none needed an edit.

## R2 — what moved

`git mv` of the three files into a new `no_std` crate **`fjell-frame-alloc`** (`publish = false`,
`#![forbid(unsafe_code)]`, one dependency, the zero-dependency `fjell-abi`). The kernel's `mm/mod.rs`
re-exports `address`, `error` and `frame_alloc`, so **no other kernel file changed** (one comment in
`task/id.rs` that named the old path). Only `frame_alloc.rs` changed in the move beyond its imports'
module paths, which still resolve: the tests got `use std::{boxed::Box, vec, vec::Vec}` (the crate
is `no_std`; a test build is the one place `std` is fine). **No `unsafe`:** a bitmap over a caller's
`&mut [u64]` needs none, and the new crate forbids it. I took the wide cut the RFC proposed; a
narrower one would have left `PhysFrame` in the kernel and the allocator unable to name its own
return type.

## R3 / R4 — the count, and whether anything was wired

**Nothing needed wiring.** `cargo test --workspace --lib` (the host-lib tier) takes workspace members,
and the new crate is one. Before: **742 passed across 54 lib test binaries**. After: **753 across 55,
1 ignored**. +11 = the six tests the file already had (no gate had run them) + five new. The number moved, so the crate was picked up. CI's `ci-test-lib` runs the same
`cargo xtask host-lib-tests`, so no workflow change.

## R5 — the deliverable

Five tests assert the property RFC-0.39-003's boot invariant enforces — *a reserved range contains
no free frame* — on E-097's own layout in miniature (image = frames 0..8, stack = 8..40, and the
16-frame "boot window" that began where the stack did):

| Test | What it shows |
|---|---|
| `e097_a_stack_reserved_by_the_coincidence_of_a_window_leaves_frames_free` | **builds the defect** and asserts the invariant sees it: `first_free_in` finds the stack's 17th frame, and `alloc_frame` really hands it out |
| `e097_the_whole_stack_reserved_leaves_nothing_free_inside_it` | the fixed layout: nothing free, and nothing in that range is ever allocated |
| `a_range_reserved_across_two_calls_leaves_nothing_free_inside_it` | adjacent halves, either order |
| `a_range_reserved_after_allocation_has_begun_leaves_nothing_free_inside_it` | allocation first (next-fit hint moved), then a reservation above it |
| `first_free_in_agrees_with_what_alloc_frame_can_return` | the invariant speaks about the frames the allocator hands out, exactly |

**Demonstrated failing:** with the whole-stack test's reservation cut to half the stack it fails —
`left: Some(98304)` (0x18000, frame 24), `right: None` — the E-097 shape. The scratch edit was
reverted and the revert proved.
(My first attempt at that demonstration did not apply — `cargo fmt` had reflowed the lines my edit
anchored on — and a "pass" from it would have proved nothing; I noticed because the edit script
asserts its anchor, and redid it against the real text.)

## What the new tests found — filed, not fixed

`FrameAllocator::reserve_range` stops at the first frame already in use and returns
`AlreadyReserved`, but the frames it **had not reached stay free**. Reserving `[2,6)` after `[0,4)`
returns `Err` and leaves frames 4–5 allocatable (`first_free_in(0, 6)` = `Some(0x4000)`). That is
E-097's shape one level down. Its doc comment also says it "does not fail if a frame is already
reserved with the same owner"; the code fails on any used frame, and the allocator may carry no
owner map at all. **No caller in `kmain` overlaps today** (every `reserve_range` call was read:
image, stack, boot window, DTB, MMIO — disjoint), so nothing is wrong yet, and the boot invariant
would catch the consequence. As the RFC directs it is **not fixed here**; it is an `#[ignore]`d test
that asserts the desired property and **fails when run on purpose**. Whether to make reservation
atomic, or to correct the comment, is the architect's.

## R6 — E-100 closed

Register closure note and Summary row; `v1-limitations.md` **row 16 removed** (it was the last row
and nothing else cites it); a dated note beside the older limitations passage that listed
`mm/frame_alloc.rs` among unreachable tests. `errata-limitations` and `consistency-check --all` pass.

## R7 — gates and the three things to watch

| Watch | Result |
|---|---|
| Prebuilts moving | **all 33 byte-identical** to before (a `sha256sum` diff), as a pure re-home must leave them |
| `unsafe-audit` | **284**, unchanged — the files held none |
| `abi-snapshot` | 495 → 495, Added 0 / Removed 0 / Changed 0 — the crate is not published and not tracked |

The rest are in the review request.
