# RFC-0.39-004: A revoked frame stops being reachable

**Status:** Accepted (0.39) — accepted 2026-10-28
**Milestone:** 0.39
**Tracks.** **E-099** — `sys_dma_alloc` maps a DMA frame into the caller `R|W|U`;
`sys_dma_revoke` zeroes it and returns it to the general pool while deliberately skipping
the user-VA unmap, so the caller keeps a live `R|W|U` mapping to a frame the allocator then
hands to another task.
**Touches** *(indicative)*: `crates/fjell-kernel/src/main.rs`,
`crates/fjell-kernel/src/trap/syscall.rs`, `crates/services/fjell-neg-test/`,
`tests/qemu/profiles/dma.toml`, `docs/src/architecture/memory-model.md`,
`rfcs/ERRATA.md`.
**Relates to:** **RFC-0.39-003**, which fixed E-098's three verified defects, documented
this residual in the code and stopped at its stated non-goal; **E-097**, whose cause is the
reason this is now testable; **E-093**, the general reclamation question this does not
touch.

## Why this is 0.39's last line

E-099 is **OPEN** and Gate 7 refuses, so 0.39.0 cannot cut until it closes. Work that must
precede a cut cannot be numbered after it. It is also the most serious entry the register
has carried: **cross-task read and write**, through the documented API, by any of
`STORAGED`, `DRIVER_VIRTIO_BLK` or `NEG_TEST`.

## The machinery already exists

This is a smaller line than E-099's severity suggests, and the reason is worth stating so
nobody rebuilds what is already there:

- **`unmap_user_va_for(owner, user_va, page_count) -> bool`** exists in full
  (`main.rs:388`): it loops the pages, calls `unmap_page`, **checks each result**, and
  returns false on the first failure. Its doc comment says it is *"Called by `revoke_by_pa`
  when the deferred PTE-unmap path … is re-enabled. Currently bypassed."*
- **The region entry already stores what it needs.** `user_va` and `page_count` are
  recorded at alloc time, with doc comments saying they are *"read by `unmap_user_va_for`
  when the deferred PTE-unmap path is re-enabled."*
- **`unmap_page` returns the `PhysFrame` it unmapped** (`mm/page_table.rs:101`) and reports
  `NotMapped` rather than touching an absent mapping — which §"The design point" turns into
  the safety property.
- **`release_task` correctly needs no unmap**, and RFC-0.39-003 argued it precisely: an
  exiting or faulting task is never scheduled again, nothing reuses its page-table root,
  so there is nothing to unmap *for* it. That reasoning is accepted and this line does not
  disturb it.

So the work is two call sites and the property that makes them safe.

## The design point: a verified unmap is safe under either history

The unmap was skipped in v0.8.x because of *"page-table corruption … `revoke_by_pa` was
called with a stale or wrong `user_va`, clobbering the task's text/rodata PTEs"*, and the
root cause was never isolated. Two explanations survive:

- **(a)** the corruption was **E-097** — the kernel stack overwriting task frames,
  including text — and the unmap was blamed for it. E-097 is now fixed and verified, so
  under (a) the unmap was always innocent.
- **(b)** the unmap really was called with a wrong `user_va` and really did clobber an
  unrelated PTE.

**This line does not need to know which.** `unmap_page` returns the frame it removed, so
the caller can require that frame to be **the one the region entry says it should be** and
refuse otherwise. Under (a) the check never fires. Under (b) it fires instead of
clobbering, and says so. A verified unmap is correct under both histories, which is why
this is a line rather than an investigation.

That is the requirement to get right. "Re-enable it and see" would be a gamble; "re-enable
it and verify what you removed" is not.

## Requirements

**R1 — Re-derive, with controls.** That `sys_dma_alloc` maps `R|W|U` and the mapping
survives revoke; that `free_dma_frame` returns the frame to the **general** pool; that
`unmap_user_va_for` exists and is uncalled; that the entry stores `user_va` and
`page_count`; and the three capability holders (`STORAGED`, `DRIVER_VIRTIO_BLK`,
`NEG_TEST`). `/usr/bin/grep -a`.

**Then sweep**: any other path that frees a frame while a user mapping to it survives.
`release_task` is the one case already argued not to need it — confirm that argument still
holds rather than assuming it. **If you find a fourth path, stop and escalate.**

**R2 — The unmap is performed and verified, before the frame is zeroed or freed.** In
`revoke_by_pa` and in `sys_dma_alloc`'s rollback:

1. Unmap each page of the region in the owner's page table.
2. **Require the frame each unmap returns to be the frame the region entry records.** A
   mismatch means the VA was wrong — the exact failure v0.8.x blamed. **Report it and
   refuse: do not zero, do not free.** One frame leaks, loudly, which is the trade
   RFC-0.39-003 already established for this family.
3. Only then zero, and only then free.

**State the ordering you implemented and why**, including what happens on a partial
multi-page unmap.

**R3 — `map_page`'s discarded result, in the same function.** `sys_dma_alloc` does
`let _ = map_page(...)` per page. If a map fails, the syscall returns success with a
missing or partial mapping, and the rollback then unmaps pages that were never mapped.
Check it, and fail the allocation closed. This is the **E-014** family in the function R2
is already editing; it is in scope for that reason and no other.

**R4 — A tier that demonstrates the hole closing, and the assertion is the mapping.**
`NEG_TEST` already holds `DmaRegion` with `ALL_NON_META`, so no capability change is
needed — which is the point, since granting one would widen the surface in the line that
narrows it.

The property to assert is **that the revoking task can no longer reach the frame**: after
`sys_dma_revoke`, a read of the revoked VA must fault rather than succeed. Pre-fix it
succeeds and returns zeros (the frame was zeroed), which is precisely the shape that makes
this defect easy to miss — *the read works and the data looks innocent*.

Do **not** attempt to assert "the task observed another task's data": arranging a specific
later owner is not deterministic, and a tier that depends on allocation order would be
flaky in the way 0.39 has just spent a line curing. The mapping's survival **is** the
defect; assert its absence.

**R5 — Demonstrated failing.** R4's tier red against the pre-change tree — where the read
after revoke succeeds — and green after. Both transcripts in the review request.

**R6 — The three places that currently document the hazard must stop describing it as
present**, and must not be rewritten to pretend it never was:
`sys_dma_revoke`'s doc comment (*"User VA unmap deferred to v0.3 … VA stays mapped"*), the
alloc rollback's comment, and `revoke_by_pa`'s. Each becomes a statement of what the code
now does, with a dated note where it is a published record. `memory-model.md`'s MM-PHY
invariants gain the new one.

**R7 — E-099 closes in this line's commit.** It is the last OPEN entry, so Gate 7 goes
green and 0.39.0 becomes cuttable. **Do not touch any other register entry to help that
along** — if a gate is still red after E-099 closes, that is a finding and I want it.

**R8 — Gates by exit status.** `release-rehearsal` (expect **Gate 7 green** for the first
time this milestone), `consistency-check --all`, `readiness-check`,
`abi-snapshot --verify`, `fjell-repro-check`, `test-all` to completion, `callsite-audit`,
`cargo fmt --check`. A kernel change moves placements: rebuild **before** re-recording the
repro baseline and commit the binaries with it. One `test-all` at a time.

**Run the `dma` and `semantic` tiers more than once.** `semantic` because this milestone
has one intermittent-corruption scar already and a page-table change is exactly where a
second would hide; `dma` because it is the tier this line edits.

### Non-goals

- **E-093**, reclaiming task memory in general; **E-100**, the ungated allocator tests;
  **E-094** and **E-095**, the measurement residuals.
- **`release_task`'s unmap.** Argued unnecessary, accepted; confirm, do not change.
- **Intermediate page-table pages.** `unmap_page` deliberately does not free them. Leave
  that.
- **Any addition to the kernel's ABI surface.** Escalate.

## Risks

- **An unverified unmap.** The whole line rests on R2's step 2. Without it this is the
  v0.8.x change again, and the argument for making it would be no better than it was then.
- **A tier that asserts a successful read of zeros.** Pre-fix the read *succeeds*; a tier
  checking "the data is zero" would pass before and after and prove nothing. R4 asserts the
  fault.
- **Fixing the symptom at the wrong layer.** Unmapping at the *user VA* is the fix;
  quarantining the frame instead, or declining to free it, would hide the stale mapping
  rather than remove it and would leak a frame per revoke.
- **Partial multi-page state.** A region can be several pages. Decide and state what
  happens when page 3 of 4 fails to unmap; the answer must leave nothing freed.
