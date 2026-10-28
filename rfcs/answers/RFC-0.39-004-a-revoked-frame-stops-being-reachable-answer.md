# RFC-0.39-004 R1–R8: a revoked frame stops being reachable

**Governing RFC:** [../done/RFC-0.39-004-a-revoked-frame-stops-being-reachable.md](../done/RFC-0.39-004-a-revoked-frame-stops-being-reachable.md)
**Handoff:** [../handoffs/RFC-0.39-004-a-revoked-frame-stops-being-reachable/implementation-handoff.md](../handoffs/RFC-0.39-004-a-revoked-frame-stops-being-reachable/implementation-handoff.md)

Written against base `d2f29ec`. Order followed: R1, the tier red (R4/R5), R2, R3, the tier green, R6, R7,
R8. **E-099 closes in this line** (R7).

---

## R1 — re-derived, with controls; the sweep

* `sys_dma_alloc` maps the frame `R|W|U` into the caller (`trap/syscall.rs`); before this line
  `revoke_by_pa` → `free_dma_frame` zeroed it and returned it to the general pool and **never
  touched the mapping**. `unmap_user_va_for` existed, was `#[allow(dead_code)]`, and had no caller.
  The region entry stores `user_va` and `page_count`. The three `DmaRegion` holders are
  `STORAGED`, `DRIVER_VIRTIO_BLK`, `NEG_TEST` (`spawn.rs`).
* **Sweep for a fourth path. Command:** `git grep 'free_frame('` over `crates/fjell-kernel/src`.
  **Finding:** one caller, `free_dma_frame` — so *no other path frees a frame at all* and none can free
  one while a mapping survives. `unmap_page`'s other callers are `vspace.rs` and `sys_mmio_map`'s own
  rollback (device frames, never freed). Control: `map_page(` finds 7 sites in `spawn.rs`.
  **Boundary:** this covers frees; it does not claim every PTE is correct. **No fourth path.**
* **`release_task`'s argument re-confirmed, not changed:** an exiting or faulting task is never
  scheduled again, no page table is torn down or reused, and `free_dma_frame` is the only freer — so
  there is nothing to unmap for it.

## R4/R5 — the tier, red first, and what it asserts

**The assertion is the mapping's absence, not the data.** After `sys_dma_revoke`, a read of the revoked
VA must *fault*; the evidence is the kernel's `LoadPageFault` line, because the task cannot print after
taking the fault. A tier checking that the data is zero passes before and after: the revoke zeroes the
frame, so a surviving mapping returns innocent-looking zeros.

The fault ends `neg-test`, and `neg-test` runs the scenarios of every profile, so it is **triggered**:
`init` has a new console byte `V` (→ `neg_test::RUN_DMA_REVOKED_READ`, `0x5F1`), only the `dma` profile
injects it, and the scenario runs after all the others, in the slot `neg-test` already parks in waiting
for `R`. No capability was granted to anyone.

* **Pre-fix (red, exit 1):** `init: dma-revoked-read trigger received` → `NEG:DMA:REVOKED_VA_READ_ATTEMPTED`
  → **`NEG:HARNESS:UNEXPECTED_OK`** (the read succeeded) and no fault line. `once-marker check FAILED —
  neg-test]: fault(LoadPageFault) … never appeared`; `FORBIDDEN marker NEG:HARNESS:UNEXPECTED_OK`.
* **Post-fix (green, 5 markers + the fault line exactly once):**
  `neg-test: reading revoked dma va=1610625024` then
  `[task#7 neg-test]: fault(LoadPageFault) sepc=0x41a64 stval=0x60003000`. **`1610625024 = 0x60003000`**:
  the fault is from that read, not from something else `neg-test` did.

## R2 — the ordering implemented, and why

`revoke_entry` (one implementation for `revoke_by_pa` and the unused `revoke_by_id`) and the alloc
rollback run:

1. **read** the owner's PTE (`translate`) and require it to be the frame the region records;
2. **unmap** it and require the frame removed to equal it again;
3. **flush** the TLB;
4. **zero** the frame and **read it back as zero**;
5. **free** it.

**Where my design differs from the RFC's, and why.** R2 says unmap, then require the returned frame to
match. But `unmap_page` **clears the PTE before it returns the frame**, so "unmap, then check" has
already clobbered a wrong PTE when the check fires — the failure v0.8.x blamed the unmap for. Reading
the PTE first (step 1) is what makes the check *instead of* clobbering and not after it. The second
check is belt and braces.

**On failure.** Step 1 or 2 failing → `Unverified`: **nothing is zeroed or freed**, the page table is as
found, the region stays Active, the task keeps its own frame and mapping, the syscall returns
`InternalError`, the kernel says why. (If the task later exits, `release_task` zeroes and frees it.)
Unmapped but zero/free refused → `NotReleased`: the task can no longer reach the frame, one frame
leaks, loudly. **Multi-page:** a region whose `page_count != 1` is refused outright — the entry records
only the first frame, so the others could not be verified, and `DMA-003` allows one page. There is no
partial state to decide on because there is no partial attempt.

**Was the frame-identity check ever fired? No.** Not in the 30 targeted runs, not in `test-all`: no
`kernel: DMA …` line appeared anywhere. That is what "the v0.8.x corruption was E-097 and the unmap was
innocent" predicts; the other explanation predicts a firing. It supports the first, it does not prove
it.

## R3 — `map_page`'s discarded result

`sys_dma_alloc` now checks it. `map_page` installs the leaf PTE as its last step, so a failure leaves
nothing mapped; the never-visible frame is freed through the checked helper and the call returns
`NoMemory`, with a console line. Before, a failed map returned success with no mapping.

## What the RFC did not name — the zeroize scenario depended on the hole

`dma_zeroize` read the frame back **through the revoked address** and expected zeros; it worked only
because the mapping survived. It cannot any longer (the read faults). The zeroing is now verified where
it *can* be observed: `free_dma_frame` reads the frame back after zeroing and refuses to free it if any
byte is non-zero, and `NEG:DMA:ZEROIZE_ON_EXIT` is emitted when `sys_dma_revoke` returns `Ok` — which
now means unmapped, verified zero, and freed. The marker keeps its name; its basis moved. The kernel's
refusal lines are added to the harness's closed forbidden set, so one appearing fails any tier.

## R6 — the three places

`sys_dma_revoke`'s doc, the alloc rollback's comment and `revoke_by_pa`'s doc now say what the code does,
and keep the history ("this comment used to say…"). A fourth, found by the sweep: `docs/src/external-design/kernel.md`'s
known-gaps row said *"Stale PTE, mitigated by zeroize-before-reuse"* — the false claim itself; it is
struck through with a dated note. `memory-model.md` gains MM-PHY-008.

## R7 — E-099 closed

ERRATA closure note and Summary row `CLOSED`; no other register entry was touched.

## R8 — gates

See the review request.
