# RFC-0.30-002 §5 — When is the ABI enumeration owed?

**Governing RFC:** [rfcs/accepted/RFC-0.30-002-checks-that-name-themselves.md](../../rfcs/accepted/RFC-0.30-002-checks-that-name-themselves.md)

Answered after reproducing E-038 and building R1/R2, per the handoff's
required order, and before touching E-035's code.

---

## Answer: shape 1 — every addition, enforced where it happens

`fjell-abi-snapshot --verify` now fails on `Added != 0`, not only on
`Removed`/`Changed sig`. Gate 4 needed no change of its own: it already
reads the tool's `Result: PASS`/`FAIL` line rather than re-deriving a
verdict, so it inherits the stricter behaviour for free.

## The cost, argued rather than assumed

**Measured, not guessed:** `tests/abi/snapshot.json` has been touched in
**8 commits** across this project's entire history (`git log --oneline --
tests/abi/snapshot.json`), against **177** RFCs shipped to `rfcs/done/`.
The stable surface changes rarely — both RFC-0.30-001 and this RFC itself
declare "does not touch... the ABI surface" as a matter of course, and
that is the common case, not the exception, measured directly rather than
assumed from how the RFC's own risk section framed it.

**What "routinely red" would require, and why it doesn't apply here:** a
gate becomes background noise when it fires often enough that seeing it
red stops being informative, or when the person who can act on it is not
the person who sees it. Neither holds for shape 1 as built:

- It fires only on the ~4.5% of lines (8/177, and falling as the ABI
  surface has matured) that touch a stable crate at all.
- It fires on the *same commit* that made the addition, for the *same
  author* who has the context — not a downstream job, not a nightly run,
  not someone else's problem inherited at a cut. There is no window in
  which the gate is red on `main` for a change some other line made; the
  discipline is "regenerate before your own line is done," which this
  project already applies identically to `docs/release/trust-report.txt`.
- Unlike a check that stays red until someone finds time (the shape that
  genuinely breeds ignoring it), this one is closed by one mechanical
  command (`--generate`) the author already has open. The friction is a
  single extra step, not an open-ended fix.

**The real cost is smaller than the RFC's own risk section assumed**, in
the same shape as RFC-0.30-001's R1 finding: a number nobody had measured
turned out to argue for the placement everyone assumed was expensive.

## Shapes 2 and 3, addressed

**Shape 2 (cut-only)** is not built, matching the RFC's own framing: no
mechanism today knows it is a cut, and building one is more machinery than
four small edits and a stricter `if`. It is also strictly worse than shape
1 at the one thing that matters — RFC-0.24-003's whole argument is that
review happens where the knowledge is, and the cut is never where an
addition was made.

**Shape 3 (stamp the baseline with a version, fail on drift)** solves a
narrower problem than D2/R1 asks for: it would catch "the baseline is
older than the workspace," which is a proxy for "an addition might be
unrecorded," not the addition itself. A line that adds an item and
regenerates *within* the same milestone, correctly, would still trip a
version-drift check for no reason, and a line that adds an item without
regenerating at all would not trip it until the next version bump —
later than shape 1, not sooner. Not attempted; shape 1 answers the actual
question directly instead of inferring it from a proxy.

## What changed

`tools/fjell-abi-snapshot/src/main.rs`: `verify()`'s pass condition is now
`removed.is_empty() && changed.is_empty() && added_count == 0`; the
`FAIL` path enumerates every added item by name when `added_count > 0`
(previously only `Removed`/`Changed` were itemized).
`docs/src/release/v0-release-cycle.md`: the "before criterion 6" step
rewritten from a cut-time task ("enumerate here") to a cut-time
*confirmation* that the per-line discipline already held — a non-zero
`Added` at the cut is now itself a finding (the gate should have already
refused the merge that caused it), not routine cut work.
