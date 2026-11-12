# Developer Handoff — RFC-0.43-002

**Governing RFC:** [RFC-0.43-002](../../done/RFC-0.43-002-every-declaration-is-visible-to-the-instrument-that-checks-it.md)
**Milestone:** 0.43 · independent of 0.43-001
**Status:** inherited from the governing RFC (Implemented, 0.43.0)
**Audience:** implementation model

---

## 1. Read the RFC first

Six requirements, and **R5's two demonstrations are what finish the line.** One of them is the
`CAP_ARCH_PROBE_12TH` control replayed on a new axis: this module was rewritten precisely because
a planted constant passed silently against an enumerated list, and today a planted constant in
`crates/drivers/` passes for the same reason. Show it caught.

**E-117 is OPEN and scheduled to this line**, which means the tree cannot be cut until it closes.
That is deliberate, not pressure to close it quickly.

## 2. The line in the sand

**R1 is not "add `crates/drivers`".** Replacing one hard-coded directory with two is the
enumerated list the module's own doc rejects, one element longer — and it would leave the next
crate laid out somewhere new invisible all over again. Discovery must reach the declaration
*wherever it is written under `crates/`*.

**Do not clear a red by widening `WAIVERS`.** A waiver needs a reason that is true. If you end
with more waivers than fixes, that may be the correct outcome — **say so explicitly in the review
request** so the review can judge whether the check found the truth or was talked out of it.

## 3. Yours to argue, not mine to dictate

1. **How `match` arms are attributed (R2).** I have said *what* must be true — an image with an
   `ep_obj` arm has slot 0 installed, an image without one does not — and not how to read it. If
   you find a shape you cannot attribute confidently, **fail closed on that shape and name it**
   rather than guessing; an over-attributing parser reports an install that does not exist, which
   is worse than the exclusion it replaces.
2. **Whether slot 0 needs a waiver category of its own.** Runtime-delivered slots (R4) are a
   different kind of "not installed at spawn" than "nobody installs this at all". If the two want
   distinguishing in `WAIVERS`, argue it.

## 4. Mechanics and traps

- **`capability_topology.rs` is pure over its inputs** (`judge_sources`), so its rules are
  unit-testable on the host. Use that: the parser changes in R1/R2 are exactly what host tests
  should pin.
- **`read_dir_named` is the existing helper** the module already uses for discovery; prefer it to
  a new traversal so failures report through the same path.
- **The module doc is part of the deliverable.** It carries the false sentence about the `_ => 0`
  fallback (R2). A correct implementation under a doc that still cites a deleted mechanism is not
  finished.
- **`driver-uart` is your control for R3**, not this handoff: `CapHandle(32)` for region 1 under
  the `31 + region_idx` rule, *and it uses it* in `sys_mmio_map`. Derive virtio-net's number the
  same way and report what you derived. **If it is not 34, report that rather than writing 34.**
- **`grep` here skips files containing NUL bytes** — use `/usr/bin/grep -a` when sweeping
  `crates/` for declarations, or a prebuilt `.bin` in the path will silently drop a file.
- `consistency-check` is Gate 12 of `release-rehearsal`; run `cargo xtask consistency-check --all`
  and not only your own subcheck, because the 21 subchecks share parsing helpers.

## 5. Evidence the review request must carry

1. **The widened check red on today's tree**, quoted, naming virtio-net's `CAP_MMIO` — taken
   **before** R3 is applied.
2. **The planted-constant control**: a `CapHandle` constant added under `crates/drivers/`, the
   check catching it, quoted, and the plant removed.
3. **The full population**: all nineteen declarations with, for each, installed / waived / defect.
   Nine were previously unchecked and R6 says report every one, including anything the RFC did not
   predict.
4. **Slot 0 before and after**: what the check said about slot 0 when it was excluded, and what it
   says now, so the exclusion's removal is visible rather than asserted.
5. **The derivation for R3**, shown — region index, the `31 +` rule, `driver-uart` as the control
   — not just the final number.
6. **`consistency-check --all` green at the end**, with the subcheck count, and the host tests you
   added for the parser.

## 6. Stop and escalate if

- The widened universe surfaces a cluster rather than the predicted single defect. **Report it;
  do not narrow R1 until the cluster disappears.**
- Attributing the `match` arms would require the module to understand control flow it currently
  does not. That is a real boundary and changes R2's shape.
- Correcting `CAP_MMIO` turns anything red — it should not, because nothing uses it, and if
  something does then the declaration was load-bearing after all, which is a different finding.

## 7. Not yours

`rfcs/ERRATA.md` (**E-117**'s closure) is the architect's. So is any `v1-limitations.md` row.
**`proxy-text`'s slot 2 is explicitly not yours to touch** — it is correct, it is an introspection
subject read by `sys_cap_inspect`, and E-117 withdrew the claim that it was a defect. If your
widened check flags it, that is a finding about the check's predicate, not about proxy-text.
