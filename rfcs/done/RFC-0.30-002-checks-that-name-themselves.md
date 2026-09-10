# RFC-0.30-002: Four subchecks that fail without saying which, and a baseline step nothing enforces

**Status:** Accepted — by the owner (nabbisen), 2026-09-09; implementation may begin (RFC 000)
**Milestone:** 0.30
**Tracks.** **E-038** and **E-035** — the two errata dated `0.30`, both of which
slipped out of 0.29 and were refused by `errata-tracking` at that cut. Doing them
now keeps the milestone's word rather than slipping them twice.
**Touches.** `tools/fjell-consistency-check`, `tools/fjell-abi-snapshot` and/or
`crates/fjell-tools`'s Gate 4, the release cycle. **Does not touch the kernel,
the ABI surface, or any service.**
**Relates to:** RFC-0.24-003 (the ABI-snapshot discipline E-035 is about);
RFC-0.29-002 (which repaired seven predicates and did not reach these);
RFC-v0.22-001.

## Summary

Two small instrument defects, both committed to this milestone, both already
slipped once.

### E-038 — and it is four subchecks, not three

Reproduced by moving `rfcs/accepted/` aside and running the real tool:

| Subcheck | With the folder absent |
|---|---|
| `rfc-status-folder` | header, then `consistency-check: cannot read rfcs/accepted` — **no result line** |
| `handoff-status` | **header and nothing else at all** |
| `errata-tracking` | header, then `consistency-check: cannot read rfcs/accepted` — **no result line** |
| `doc-counts` | header, then `cannot read one or more rfcs/ lifecycle folders` — **no result line** |

The erratum says *"three subchecks emit no output at all."* **Both halves are
wrong.** Three emit a message; what none of them emits is a **result line naming
itself**, which is the format every passing subcheck uses and the thing a reader
scans for. And there is a **fourth** — `handoff-status` — which this table never named.

> **Correction, architect, 2026-09-10, at the implementation review.** The row
> above says `handoff-status` produces "header and nothing else at all", and the
> sentence after it calls it "the only one that really is silent." **Both are
> wrong, and the error was mine.** The reproduction behind this table filtered
> the tool's output on the literal string `cannot read`; `handoff-status`'s
> message reads `... which could not be read`, so my own predicate hid the
> evidence — the same defeated-by-literal-matching shape this RFC and
> RFC-0.29-002 exist to fix, committed inside the RFC that names it. R2's
> diagnosis found what actually happens: `handoff-status` never enumerated the
> lifecycle folders at all, reaching them only through whichever RFC a handoff
> happened to cite, so with `rfcs/accepted/` absent it either printed an unnamed
> message or **passed** — a false PASS, worse than the silence I attributed to
> it. Recorded here rather than edited away; the requirements below stand
> unchanged, and R2 is the reason the real defect was found.

The aggregate ends `consistency-check: FAIL` with no `<name>: FAIL` anywhere, so
the reader gets a failure with no subject. Three identical `cannot read
rfcs/accepted` lines do not say which check produced them.

### E-035 — the ABI baseline step is a paragraph

`tests/abi/snapshot.json` opens `{"count":419}` and records nothing else about
itself — not the version it was taken at, not when, not by which line. The
release cycle gained a step at the 0.28.0 cut: enumerate the additions, name the
RFC behind each, then regenerate. **Nothing enforces it.**

`fjell-abi-snapshot`'s own doc says *"items added between snapshots are **not** a
failure (additive change)"*, and no gate acts on `Added`. So the baseline can
drift additively for as long as nobody chooses to run the step, and **whenever
someone finally regenerates, every accumulated addition is absorbed in one
unreviewed commit** — RFC-0.24-003's hazard, reached by patience.

It currently reads `Added: 0` only because the 0.28.0 and 0.29.0 cuts happened
to do it by hand.

## The settled part

**D1 — Every subcheck ends with a result line naming itself**, pass or fail.
`<name>: FAIL — <reason>` is the format; a bare `cannot read` line is not it.
The point is not the message, it is that **the reader can tell which of ten
checks failed.**

**D2 — Fix the class, not the four.** Whatever mechanism guarantees a named
result line should make it structurally hard for an eleventh subcheck to be
added without one. Four separate edits leave the next author to remember.

**D3 — The `handoff-status` silence is a separate bug from the other three** and
must be understood before it is fixed. The other three print something and
return; this one prints nothing. Find out why before making it match.

**D4 — E-035's fix must not fail a tree that is legitimately mid-milestone**
unless §5 decides it should. An RFC that adds an ABI item is normal; the
question is when the enumeration is owed.

**D5 — Demonstrated failing** (RFC-v0.22-001): for E-038, a missing folder must
produce a named result line from each of the four. For E-035, a deliberately
un-regenerated baseline must be caught by whatever is built.

## The open question — §5

**When is the ABI enumeration owed?**

1. **At every addition.** Any `Added != 0` fails Gate 4, so the enumerate-and-
   regenerate discipline happens in the line that adds the item, which is where
   the knowledge is. Simplest to enforce and hardest to defer. Cost: every
   ABI-adding RFC must regenerate, and `release-rehearsal` goes red between the
   addition and the regeneration.
2. **At the cut only**, needing a mechanism that knows it is a cut — which does
   not exist today, and inventing one is more machinery than the problem.
3. **Stamp the baseline** with the version it was recorded at, and fail when the
   workspace version has moved past it. Enforces the *cut* step without needing
   to know about cuts. Cost: a new field, and it says nothing about additions
   made and regenerated within one milestone.

**Answer in writing before implementing.** I lean to 1 — it puts the enumeration
in the line that has the context, and RFC-0.24-003's whole argument is that
regeneration must be reviewed where it happens. But 1 makes the gate red during
normal work, which is a real cost and may be the reason it was never done that
way.

## Requirements

**R1 — All four subchecks emit a named result line** on a missing folder, per D1
and D2.
**R2 — `handoff-status`'s silence diagnosed** before it is fixed (D3).
**R3 — §5 answered**, and E-035's enforcement built accordingly.
**R4 — Both demonstrated failing** (D5).
**R5 — E-035 and E-038 → `CLOSED`**, register and `v1-limitations.md` in the
same commit. **E-038's text is corrected**, not just closed: it says three
subchecks and no output; it is four and no *result line*.

### Non-goals

- **E-037** — the toolchain. It is the larger remaining piece and gets its own
  line.
- E-014's two survivors, E-034, E-039.
- Changing what any check decides.
- Any kernel, ABI-surface, or service change.

## Risks

**These are small, which is how they slipped once already.** They were dated
0.29, nothing forced them, and larger work displaced them. The same will happen
again unless they go first in 0.30 rather than last.

**§5 shape 1 makes `release-rehearsal` red during ordinary work**, and a gate
that is routinely red is a gate people learn to ignore. If that is the objection
that killed it before, it deserves stating rather than rediscovering.
