# RFC-0.41-004: Frame reservation is all-or-nothing, and its doc says what it does

**Status:** Accepted — accepted 2026-10-30
**Milestone:** 0.41
**Tracks.** **E-103** — `FrameAllocator::reserve_range` is not all-or-nothing in either
direction, and its doc comment describes a same-owner tolerance the code does not implement.
**Touches** *(indicative)*: `crates/fjell-frame-alloc/src/frame_alloc.rs`,
`crates/fjell-frame-alloc/src/lib.rs`, `rfcs/ERRATA.md`.
**Relates to:** **RFC-0.40-003**, which moved these tests into a crate a gate can run and whose
implementer found the first divergence; **RFC-0.39-003**, which added the owner map this line can
corrupt and whose boot invariant currently backstops the consequence; **E-097**, the same shape one
level down; **E-100**, closed — the reason an `#[ignore]`d test is treated as absent coverage.

## What is wrong

Three divergences in one function, observed in `crates/fjell-frame-alloc/src/frame_alloc.rs`.

**1. The unreached tail stays free.** `reserve_range` returns `AlreadyReserved` at the first used
frame, leaving frames it never reached **free** inside a range the caller asked to reserve.
Reserve `[0,4)`, then `[2,6)`: `Err`, and frames 4–5 are allocatable.

**2. The reached head is consumed and relabelled.** The frames *before* the conflict are already
marked used, with the owner map overwritten, when the call returns `Err`. Reserve `[4,8)` as
`KernelText`, then `[0,8)` as `KernelStack`: the call fails, `free_count` drops by 4, and frames
0–3 report `owner_of == KernelStack` — the map RFC-0.39-003 added precisely so a frame's holder
could be named. The loop sets the used bit and the owner before it reaches the conflict; the
architect's probe demonstrated it rather than inferring it from the source.

**So a caller that treats `Err` as "nothing happened" is wrong in both directions at once** — some
of the range it asked for is still free, and some of it is now used and mislabelled.

**3. The documented contract is not the implemented one.** The doc says the call *"does not fail
if a frame is already reserved with the same owner, but returns `AlreadyReserved` for conflicting
owners"*. The check never reads `owner` and fails on **any** used frame, so a same-owner
reservation fails. A reader who trusts the comment will write a caller that cannot work.

**Latent, not live.** No kernel caller overlaps today — every reservation in `kmain` was read, and
image, stack, boot window, DTB and MMIO are disjoint — and RFC-0.39-003's boot invariant backstops
the consequence of the tail. That is why this is scheduled rather than urgent, and it is also why
it must not be left: the invariant is doing work the allocator should be doing itself.

**The coverage that exists is not evidence.**
`an_overlapping_reservation_does_not_leave_the_rest_of_its_range_free` asserts divergence 1's
property, is `#[ignore]`d, and **fails when run on purpose**. It covers divergence 1 only.

## Requirements

**R1 — validate the whole range, then apply.** Make the reservation all-or-nothing: check every
frame in the range first and return `Err` before mutating anything, so a failed call leaves the
allocator **bit-for-bit untouched** — used bits, `free_count` and the owner map alike. One change
of shape fixes divergences 1 and 2 together, which is why it is the ruled direction rather than
patching the tail.

**R2 — do not implement the same-owner tolerance. Correct the comment instead.** No caller needs
it, and adding behaviour so that code matches a comment is the wrong direction of travel. Say
plainly that any already-used frame fails, whatever its owner.

*This is a deliberate ruling, not an omission: if a future caller needs same-owner idempotence it
can be added then, with a test and a reason. Today it would be untested behaviour existing only to
justify a sentence.*

**R3 — un-`#[ignore]` the existing test, and cover divergences 2 and 3 too.** The ignored test
becomes a running test. Add:

* **divergence 2**: after a failed overlapping reservation, assert `free_count` is unchanged **and**
  `owner_of` on the head frames is unchanged — both halves, because the probe found both.
* **divergence 3**: assert that a same-owner overlapping reservation **fails**, which is now the
  documented contract.

**R4 — run the failure case, not only the absence.** Each test must be shown to fail against the
current implementation before the fix lands, and pass after. A test written against the fixed code
that was never seen red is not evidence that it tests anything. Record which tests were seen red.

**R5 — the ignore reason and the erratum stop naming each other.** The `#[ignore]` reason names
E-103 so each could be found from the other. When the ignore goes, the erratum's closure note takes
over that job; make sure the entry says where the coverage now lives.

**R6 — the host-lib tier count moves.** It reads 761 passing / 55 / 1 ignored today. Un-ignoring
one test and adding three moves both the passing count and the ignored count to zero. State the
new figures in the record; do not let a count drift silently, which is the `doc-counts` class of
defect one level out.

### Non-goals

* **Atomicity against concurrency.** This is single-hart and the fix is about *transactional*
  behaviour within one call, not about locking. "Frame reservation is still not atomic" in the
  concurrency sense stays a stated limitation.
* **E-097.** The same shape one level down, already closed; this line does not revisit it.
* **Removing RFC-0.39-003's boot invariant.** It backstops more than this, and a correct allocator
  is not a reason to delete a check that would catch the next defect.
* **Widening `reserve_range`'s signature.** No new parameters; the contract gets *narrower* and
  truthful, not more configurable.

## Risks

**R1 touches the boot path.** Every reservation in `kmain` goes through this function. A
validate-then-apply rewrite that gets the range arithmetic wrong fails at boot rather than
subtly — which is the good failure mode, but it means the QEMU tiers are the real gate here, not
the host tests alone. Run both.

**The `no_std` crate forbids `unsafe`** (RFC-0.40-003). Keep it that way; a two-pass implementation
needs no new primitive.

**A caller may have been relying on the partial effect** without anyone noticing, since `Err`
currently consumes frames. R1 makes a previously-consumed range stay free, so a caller that
double-reserved and got away with it will now behave differently. No such caller was found — every
`kmain` reservation was read — but the QEMU tiers are what would show one, so a green host-lib tier
is not sufficient evidence on its own.

## Amendments — 2026-10-31, architect, after review

**R3's first instruction contradicted R1, and the error is the architect's.** R3 said to un-`#[ignore]`
`an_overlapping_reservation_does_not_leave_the_rest_of_its_range_free`. That test asserts
`first_free_in(pa(0), pa(6)) == None` after a **failed** `[2,6)` over `[0,4)` — that is, that the
failed call *reserved* frames 4–5. Under R1's no-op contract the failed call leaves them free, so the
test fails permanently. R1 is controlling; the test is **replaced**, not un-ignored, by
`the_frames_a_failed_reservation_did_not_reach_are_exactly_as_they_were`.

**Divergence 1 is dissolved rather than fixed**, which is a better description of E-103's structure
than the entry's own: the free tail was a defect only beside a head that had already been consumed.
With the no-op contract a free tail is correct.

**R3's count was wrong**: it asked for three new tests; five are right — the divergence-2 test, the
general bit-for-bit state test, the divergence-1 restatement, the divergence-3 test and the
success-path control. **All-or-nothing stands**; reserving the reachable tail would be a different
RFC, and is not wanted.
