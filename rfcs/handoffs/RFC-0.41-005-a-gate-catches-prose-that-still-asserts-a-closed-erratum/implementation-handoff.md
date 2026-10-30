# Developer Handoff — RFC-0.41-005

**Governing RFC:** [RFC-0.41-005](../../accepted/RFC-0.41-005-a-gate-catches-prose-that-still-asserts-a-closed-erratum.md)
**Milestone:** 0.41 · **Independent** of 0.41-001 through -004 — no ordering constraint
**Status:** inherited from the governing RFC (Accepted, 0.41)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design conflict,
**stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line

**Would this check have caught the two sentences 0.40.0 shipped?** That is the whole test of it.
Both are known, both are already fixed in `8ba6762`, and both are reconstructible. If the check
does not name them when they are put back, it does not work, whatever else it passes.

**Not the measure:** that `consistency-check --all` is green. It is green now, with the gap open.

## 0.1 Traps

1. **The tree is already fixed, so a passing check proves nothing.** This line's evidence is the
   *failing* case. Budget your time accordingly: the reconstruction is the deliverable, not the
   subcheck.
2. **The stale clause sits inside a correct sentence.** `diagnosticsd`'s comment said the record
   "is independent constants, E-094" **and** that `last_attestation_available` stays `false` — the
   second half was true. A rule that needs the whole sentence to be wrong will not fire.
3. **Most mentions of closed errata are legitimate** and must keep passing — `closes E-094`,
   `closed 2026-10-29 by RFC-0.40-002`, *"that would be E-094 with extra steps"*
   (`fjell-attestd/src/main.rs:208`), the `v1-limitations.md` closure rows, the ERRATA entries
   themselves. R4 is the control; enumerate them and count them.
4. **If passing the legitimate set needs a long exception list, the rule is wrong.** Report that —
   it is a real finding about the rule. **Do not widen the list to get green.** A check with twenty
   exceptions tells a reader nothing.
5. **`grep` here skips NUL-containing files** — use `/usr/bin/grep -a` for any manual survey.
6. **Natural-language meaning is not checkable.** Match forms. Where the check cannot decide, it
   must say so rather than guess: a confidently wrong check is worse than one with a stated blind
   spot.

## 1. Order of work

**Step 1 — read the two instances before designing anything.** `git show 8ba6762` gives both,
before and after. The rule must come from these, not from first principles.

**Step 2 — enumerate the legitimate references (R4).** Every mention of a CLOSED erratum in the
tree, with the form it takes. Count them. **This set defines the rule**: whatever discriminator you
choose must pass all of these and reject the two from step 1.

**Step 3 — choose and write down the rule (R2).** Which forms are accepted — closure phrasing,
dated-closure rows, contrastive mentions — and which file types are in scope. **Write the accepted
forms into the module's own doc comment**, as the other subchecks do. A check whose rule is
undocumented cannot be satisfied deliberately.

**Step 4 — implement the subcheck.** A new module in `tools/fjell-consistency-check/src/`,
registered in `main.rs` and `lib.rs` beside the existing 20. Read `rfcs/ERRATA.md` for status;
**do not edit it** (R5) and do not add per-erratum annotations to make the check easier — that
moves the burden onto the record the check protects.

**Step 5 — the failing case, both instances (R3).** Reconstruct `v1-readiness.md:48` as it stood
before `8ba6762`; run the check; confirm it **fails and names that line**; restore. Repeat for the
`diagnosticsd` comment. Record both transcripts.

**Step 6 — the count (R6).** `consistency-check --all` reads **20 subchecks**; it becomes **21**.
Update every place that states the figure, including the subcheck list in the book. A drifting count
is the `doc-counts` class of defect one level out.

## 2. What must be shown, not asserted

* **Both reconstructions failing, with the offending line named.** Quote the output.
* **The legitimate set passing, with its count.** If it only passes via exceptions, say how many and
  stop (trap 4).
* **The documented rule**, as text, in the module.

## 3. Gates

* `cargo xtask consistency-check --all` — must read **21 subchecks** and exit 0 when you are done.
* `cargo xtask test-all` — 33 tiers; the `host-lib` tier covers the tool's own unit tests, so put
  the rule's unit tests there.
* `cargo fmt --all -- --check`.

No prebuilt should move — this is a host tool. **If one does, stop and find out why** before
re-recording any baseline.

## 4. Escalate rather than decide

* If the rule cannot separate the two instances from the legitimate set without a long exception
  list (trap 4). This is the most likely escalation and it is a legitimate outcome.
* If the right scope seems to include file types where a stale claim does not matter.
* If a CLOSED erratum's own entry would have to change to satisfy the check.
* If you find a **third** stale instance while enumerating. Report it; do not fix it silently —
  it changes what 0.40.0 is known to have shipped, which is the architect's to record.

## 5. What to report

One review request with: the enumerated legitimate set and its count; the rule as written, with its
accepted forms; both reconstruction transcripts showing the check fail and name the line; the new
subcheck count and every place you updated it; any third instance you found, reported and not
fixed; and confirmation that no prebuilt moved.
