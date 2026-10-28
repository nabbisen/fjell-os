# Developer Handoff — RFC-0.39-004

**Governing RFC:** [RFC-0.39-004](../../accepted/RFC-0.39-004-a-revoked-frame-stops-being-reachable.md)
**Milestone:** 0.39 — **the last line of it**
**Status:** inherited from the governing RFC (Accepted, 0.39)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design
conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line

**After a task revokes its DMA region, can it still reach the frame?** Today it can: the
frame is zeroed, freed into the general pool, handed to another task, and the revoking task
still holds an `R|W|U` mapping to it. When a read of the revoked VA **faults**, this line
is done.

**Not the measure:** that the read returns zeros. It already does — the revoke zeroes the
frame before freeing it. A successful read of innocent-looking zeros is what this defect
looks like from inside the attacking task, and it is why it survived since v0.8.x.

## 0.1 Almost everything you need is already written

Read these four before writing anything; between them they are most of the line.

1. **`unmap_user_va_for`** (`main.rs:388`) — loops the pages, calls `unmap_page`, checks
   each result, returns false on failure. Its own doc comment says it is *"Called by
   `revoke_by_pa` when the deferred PTE-unmap path … is re-enabled. Currently bypassed."*
2. **The region entry stores `user_va` and `page_count`** at alloc time, with doc comments
   saying they exist for exactly this function.
3. **`unmap_page`** (`mm/page_table.rs:101`) returns the `PhysFrame` it removed and reports
   `NotMapped` instead of touching an absent mapping. That return value is the safety
   property — see §0.2.
4. **`release_task`'s "Why this does not call `unmap_user_va_for`"** — the argument that an
   exiting task needs no unmap. It is correct and accepted. **Confirm it still holds; do
   not change it.**

## 0.2 The one thing to get right

The unmap was skipped in v0.8.x over *"page-table corruption … called with a stale or wrong
`user_va`, clobbering the task's text/rodata PTEs"*, never isolated. Either that was E-097
(the kernel stack, now fixed) or the VA really was wrong.

**You do not need to know which, and you must not have to.** `unmap_page` hands you the
frame it removed: require it to equal the frame the region entry records, and refuse if it
does not. Under the first history the check never fires; under the second it fires instead
of clobbering, and reports. That is why this is a line and not another investigation.

**An unverified unmap is the v0.8.x change again**, with no better argument than it had then.

## 1. Order

1. **R1**, including the sweep for a fourth path and re-confirming `release_task`'s
   argument.
2. **R4's tier, demonstrated failing (R5), before R2.** Against today's tree the read after
   revoke succeeds. Capture that — it is the evidence E-099 was real, and it cannot be
   taken afterwards.
3. **R2**, the verified unmap in `revoke_by_pa` and the alloc rollback, ordered
   unmap → verify → zero → free.
4. **R3**, `map_page`'s discarded result in the same function.
5. **Re-run R4 green.**
6. **R6**, the three hazard comments and `memory-model.md`.
7. **R7**, closing E-099, and **R8**, the gates.

## 2. Traps specific to this line

1. **Asserting the wrong thing in R4.** Pre-fix the read *succeeds and returns zeros*. A
   tier that checks the data is zero passes before and after. Assert the **fault**.
2. **Chasing "the task read another task's data".** Non-deterministic, and a tier that
   depends on allocation order is the flakiness this milestone just spent a line curing.
   The mapping's survival is the defect; assert its absence.
3. **Zeroing or freeing before the unmap is verified.** If the unmap fails or returns the
   wrong frame, nothing may be zeroed and nothing freed — one frame leaks, loudly, which is
   the trade RFC-0.39-003 already set for this family. Getting this order wrong turns a
   safety fix into a second corruption path.
4. **Partial multi-page regions.** Page 3 of 4 failing to unmap must leave nothing freed.
   Decide it deliberately and say what you decided.
5. **Quarantining instead of unmapping.** Declining to free the frame, or parking it, hides
   the stale mapping instead of removing it and leaks a frame per revoke. The fix is at the
   user VA.
6. **Do not grant `NEG_TEST` anything.** It already holds `DmaRegion` with `ALL_NON_META`.
   Granting a capability in the line that narrows the surface would be the wrong direction,
   and it is unnecessary.
7. **Run `semantic` and `dma` several times, not once.** This milestone already found one
   intermittent corruption, and a page-table change is exactly where a second would hide. A
   single green run of either is not evidence here.
8. **Rebuild before re-recording the repro baseline**, commit the `.bin`s with it.

## 3. What to put in the review request

- **R1's sweep**, with its command, control and stated boundary, and your re-confirmation
  of `release_task`'s argument.
- **R2's ordering**, and what happens on a partial multi-page unmap.
- **Whether the frame-identity check ever fired** in your testing. Either answer is
  interesting: never firing supports the E-097 explanation of v0.8.x; firing once would be
  the first direct evidence for the other.
- **What R3 does when a map fails.**
- **Both R4/R5 transcripts**, the pre-fix successful read and the post-fix fault.
- **How many times you ran `semantic` and `dma`, and the results** — counts, not "passed".
- **Exact commands and exit statuses** for every gate in R8, and **confirmation that Gate 7
  is green** — the first time this milestone, and the thing that makes 0.39.0 cuttable.
- **Any place my RFC is wrong.** Every line in 0.38 and 0.39 has found at least one, and in
  0.39-003 you corrected two of mine and a contradiction between my own requirement and my
  own non-goal. The tree wins.
