# Developer Handoff — RFC-0.41-004

**Governing RFC:** [RFC-0.41-004](../../accepted/RFC-0.41-004-frame-reservation-is-all-or-nothing.md)
**Milestone:** 0.41 · **Independent** of 0.41-001, -002, -003 and -005 — no ordering constraint
**Status:** inherited from the governing RFC (Accepted, 0.41)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design conflict,
**stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line

**Does a failed `reserve_range` leave the allocator exactly as it was?** Today it leaves the
unreached tail free *and* consumes and relabels the reached head, so a caller treating `Err` as
"nothing happened" is wrong in both directions at once. When a failed call is a no-op — used bits,
`free_count` and the owner map all unchanged — and the doc comment describes the behaviour that
exists, this line is done.

**Not the measure:** that the ignored test now passes. That test covers divergence 1 only.

## 0.1 Traps

1. **Three divergences, not one.** The tail (1), the consumed-and-relabelled head (2), and the
   documented same-owner tolerance that does not exist (3). The existing `#[ignore]`d test covers
   **1 only**. A fix validated against that test alone is incomplete.
2. **Do not implement the same-owner tolerance.** The ruled direction is to **correct the comment**.
   No caller needs the tolerance, and adding behaviour so code matches a comment is the wrong
   direction of travel. This is a decision, not an oversight.
3. **The crate forbids `unsafe`** (RFC-0.40-003). Keep it so; a two-pass implementation needs no new
   primitive.
4. **This is the boot path.** Every `kmain` reservation goes through here. The QEMU tiers are the
   real gate, not the host tests alone.
5. **A test never seen red is not evidence.** Written against fixed code it proves nothing. See §2.
6. **Re-check counts at your final tip.** The host-lib figures move, and a mid-line reading goes
   stale — that has happened here before.

## 1. Order of work

**Step 1 — the three tests first, each seen red.**

* Divergence 1: un-`#[ignore]`
  `an_overlapping_reservation_does_not_leave_the_rest_of_its_range_free`. It already fails on
  purpose — run it and record the failure.
* Divergence 2: reserve `[4,8)` as `KernelText`, then `[0,8)` as `KernelStack`; assert the call
  fails **and** `free_count` is unchanged **and** `owner_of` on frames 0–3 is unchanged. **Both
  halves** — the architect's probe found `free_count` dropping by 4 *and* the owner map
  overwritten.
* Divergence 3: assert a same-owner overlapping reservation **fails**, which is the contract after
  this line.

Record which tests you saw red. This step is the evidence for the whole line.

**Step 2 — validate the whole range, then apply (R1).** Check every frame before mutating anything,
so `Err` leaves the allocator bit-for-bit untouched. One change of shape fixes 1 and 2 together.

**Step 3 — correct the doc comment (R2).** Any already-used frame fails, whatever its owner. Say it
plainly; do not leave a hedge that implies the tolerance might exist.

**Step 4 — the `#[ignore]` reason and the erratum (R5).** The ignore reason names E-103 so each
could be found from the other. With the ignore gone, the erratum's closure note takes over that
job — make sure the entry says where the coverage now lives.

**Step 5 — the counts (R6).** `host-lib` reads **761 passing / 55 / 1 ignored** today. Un-ignoring
one and adding three moves the passing count and takes ignored to **0**. State both new figures,
measured at your final tip.

## 2. What must be shown, not asserted

* **Each test seen red, then green**, named individually. This is the requirement the architect will
  check first.
* **The QEMU tiers green**, not only `host-lib`. R1 touches the boot path, and a host-only green is
  not sufficient evidence.
* **No caller relied on the partial effect.** Every `kmain` reservation was read by the architect
  and they are disjoint, so none should. The tiers are what would show one — if a tier moves in a
  way you cannot attribute, **stop and escalate** rather than adjusting the test.

## 3. Gates

* `cargo xtask test-all` — 33 tiers. `host-lib` plus the QEMU profiles. Read the raw log of any
  tier that changes; `store` reports `success` while its log says `profile 'store' FAIL`, which is
  known and `continue-on-error` — do not let it mask a real failure.
* `cargo xtask consistency-check --all` — 20 subchecks.
* `cargo run -p fjell-repro-check` — real two-build mode, **not** the xtask's `--skip-build`
  default.

**Expected to move:** the kernel prebuilt, since `fjell-frame-alloc` compiles into it; the repro
baseline, **re-recorded in the same commit as the rebuild**; the host-lib counts.

## 4. Escalate rather than decide

* If a `kmain` caller turns out to overlap after all.
* If a QEMU tier changes behaviour you cannot attribute to this change.
* If validate-then-apply appears to need `unsafe` or a new primitive.
* If anything suggests the same-owner tolerance is actually needed — that reverses a ruling and is
  the architect's call, not a code decision.

## 5. What to report

One review request with: the three tests, each named, with the red result quoted before the green;
the new `host-lib` figures measured at your final tip; the corrected doc comment as text; which
QEMU profiles you ran; the moved prebuilt digests with the baseline re-recorded alongside; and the
two-build repro result, stated as not `--skip-build`.
