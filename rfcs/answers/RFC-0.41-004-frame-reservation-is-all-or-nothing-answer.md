# RFC-0.41-004 R1–R6: frame reservation is all-or-nothing, and its doc says what it does

**Governing RFC:** [../accepted/RFC-0.41-004-frame-reservation-is-all-or-nothing.md](../accepted/RFC-0.41-004-frame-reservation-is-all-or-nothing.md)
**Handoff:** [../handoffs/RFC-0.41-004-frame-reservation-is-all-or-nothing/implementation-handoff.md](../handoffs/RFC-0.41-004-frame-reservation-is-all-or-nothing/implementation-handoff.md)

Written against base `7747ed2`. E-103 is **not** closed by this line (see "Closure" below).

## Where the RFC is wrong — R3's first instruction cannot be followed

R3 says to un-`#[ignore]` the existing test. That test, `an_overlapping_reservation_does_not_leave_the_rest_of_its_range_free`,
asserts that after a failed `reserve_range([2,6))` over `[0,4)` the tail `[4,6)` is **not free** — that a failed
call should reserve what it can reach. R1 is the opposite contract: a failed call is a no-op, so the tail **is**
free, because the caller was told the reservation did not happen. Un-ignoring the test would make it fail
permanently under the ruled design. So the old test is **replaced**, not un-ignored, by one asserting the ruled
property (`the_frames_a_failed_reservation_did_not_reach_are_exactly_as_they_were`). Divergence 1 *as worded* is not
fixed but dissolved: it was a defect only because of divergence 2 (a caller reading `Err` as "nothing happened" was
wrong about the head); with the no-op contract the free tail is correct.

## Step 1 — the tests, each seen red (or why it could not be)

Run against the allocator as it was, before the fix:

| Test | Against the old implementation |
|---|---|
| `a_failed_reservation_neither_consumes_nor_relabels_the_head_of_its_range` (divergence 2, **both halves**: `free_count`, then `owner_of`) | **RED**: ``a failed call consumed frames  left: 8  right: 12`` — `free_count` fell by 4 |
| `a_failed_reservation_leaves_the_allocator_bit_for_bit_untouched` (conflict at the first, a middle and the last frame; with and without an owner map) | **RED**: ``conflict at frame 11, owner map false: a failed call changed state`` — bitmap `[3841]` vs `[2049]`, `free_count` 11 vs 14 |
| `a_same_owner_overlapping_reservation_fails_and_changes_nothing` (divergence 3 + the no-op) | **RED**: the owner map showed frames 2–3 relabelled `KernelStack` after the failed call |
| `the_frames_a_failed_reservation_did_not_reach_are_exactly_as_they_were` (divergence 1 under the ruled contract) | **green on the old code**, as it must be: a conflict at the *first* frame has no partial effect. It pins the contract, not a fix |
| `a_successful_reservation_still_marks_every_frame_and_ignores_unmanaged_ones` | green before and after: what the rewrite must not change |

**A slip, caught by running the red:** my first version of the bit-for-bit test passed on the old code. It took an
allocation *before* the conflicting reservation, and that allocation landed on frame 0, inside the range — so the
conflict was always at the first frame and nothing partial could happen. I moved the range to `[8,16)` and asserted
the allocation sits outside it; then it failed for the right reason. A test that cannot fail is not evidence.

**Divergence 3's refusal half cannot be red against the old code** — the code *never had* the tolerance; only the doc
claimed it. So the refusal is pinned another way: with a scratch version of `reserve_range` that **builds** the
tolerance (a used frame with the same owner does not fail), the same-owner test goes red —
``the same-owner tolerance exists again`` — and with the scratch reverted (`cmp`-identical) it is green. That is the
guard against "make the code match the old comment" coming back.

## Step 2 — validate, then apply (R1)

Two passes in `reserve_range`: the first walks the range and returns `AlreadyReserved` at the first used managed
frame **without mutating anything**; the second marks every managed frame and writes the owner. Frames outside the
managed range are still ignored (as before). No `unsafe`; the crate still `forbid`s it. The allocation hint is not
touched by either pass.

## Step 3 — the doc comment (R2), as text

> Mark a physical address range as reserved with the given owner. `start_pa` is inclusive; `end_pa` is exclusive.
> Frames outside the managed range are ignored.
>
> **All or nothing.** If *any* managed frame in the range is already in use — whatever its owner, the same owner
> included — this returns `AlreadyReserved` and the allocator is left **exactly as it was**: no frame marked, no owner
> written, `free_count` and the allocation hint unchanged. The whole range is checked before anything is mutated. A
> caller may therefore read `Err` as "nothing happened".
>
> There is deliberately no same-owner tolerance. An earlier version of this comment said a frame already reserved with
> the same owner did not fail; it did (the check never read the owner), no caller needed the tolerance, and behaviour
> added so that code matches a comment is the wrong direction of travel (E-103). If a caller ever needs idempotent
> re-reservation it can be added then, with a test and a reason.
>
> Atomic against *failure* within the call, not against concurrency: this allocator is single-hart and takes no lock.

## Step 4 / R5 — the ignore and the erratum

Nothing is `#[ignore]`d any more, so the ignore reason no longer names E-103. The erratum carries a dated progress
note that says where the coverage now lives (the five test names above). **Closure:** I did not close E-103. Closing
it would leave `v1-limitations.md` row 16 describing a live defect, and row judgement is the architect's; also, with
RFC-0.41-005's check in the tree, a closed erratum named beside `ACCEPTED` in a live document is exactly what that
check refuses. The progress note carries the closure text so it can be applied with the row.

## Step 5 / R6 — the counts, measured at the final tip

`host-lib`: **768 passed / 55 binaries / 0 ignored** (from the full run `20261030-223731`, summed from
`01-host-lib.log`). That is **763 → 768**: the old ignored test is gone (−1 ignored) and five tests are added (+5
passing; the RFC predicted three, but the divergence-2 test is one, the general state test, the divergence-1
restatement, the divergence-3 test and the success-path control are four more). `cargo test -p fjell-frame-alloc`:
16 passed, 0 ignored.

## What QEMU showed

The allocator is on the boot path, so the host tests are not the gate. A clean build, then the **full `test-all`**: 33
tiers, 32 PASS + 1 NOT-GATED (`store`), 0 FAIL. Every reservation in `kmain` still succeeds (the kernel boots and all
profiles pass), which also confirms that none of them overlapped — a caller that had relied on the partial effect
would have behaved differently. No tier moved in a way I could not attribute.

## Gates

See the review request.
