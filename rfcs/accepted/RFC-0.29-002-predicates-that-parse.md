# RFC-0.29-002: The gate that blocks releases on open errata cannot see one that has a comment

**Status:** Accepted — by the owner (nabbisen), 2026-09-09; implementation may begin (RFC 000)
**Milestone:** 0.29
**Tracks.** **E-014** (instruments deciding by fixed-string match), **E-017**
(`sound` verdicts not all demonstration-backed), and **E-015**'s two surviving
lines. All three are the same thing said three ways: *a check that did not check
what it claimed.*
**Touches.** `crates/fjell-tools` (`release_rehearsal`, `qemu_run`),
`tools/fjell-consistency-check`, `tools/fjell-unsafe-audit`,
`docs/verification/instrument-audit.md`. **Does not touch the kernel, the ABI,
or any service.**
**Relates to:** RFC-0.24-001 (which found all three); RFC-0.28-005 and
RFC-0.29-001 (the "derive, don't enumerate" template this applies to predicates);
RFC-v0.22-001 (every fix demonstrated failing).

## Summary

**Gate 7 exists to stop a release shipping with an open erratum. It cannot see
one that has a comment.**

```rust
// release_rehearsal.rs:135
let out = sh(&["grep", "-c", "| OPEN |", "docs/rfcs/ERRATA.md"]);
```

Demonstrated:

```
$ printf '| E-999 test | unscheduled | OPEN (blocked on X) |\n' > t.md
$ grep -c "| OPEN |" t.md
0
```

**And the register already writes status cells that way** — `E-004`'s reads
`| ACCEPTED (v1.0 limitation) |`. The format tolerates prose, one erratum uses
it, and the gate that must count `OPEN` matches a literal with exact single
spaces. An erratum filed as `OPEN (pending X)` would be counted zero, Gate 7
would report **`0 OPEN errata`**, and the release would proceed.

Nothing has exercised this because no erratum has been `OPEN` since the register
was created. **Gate 7 has never had to work.**

## The other live instances, verified

| Instrument | Predicate | What it misses |
|---|---|---|
| **Gate 7** | `grep -c "\| OPEN \|"` | any annotated `OPEN` — **demonstrated above** |
| **Gate 6** | `let _ = sh(…)` then counts `§1`..`§6`, passes on `>= 6` | **discards the regeneration's exit status.** A failed regeneration leaves the previous report on disk, still containing six section markers, and the gate passes on it |
| **Gate 5** | `out.contains("PASS") && out.contains("OPEN     : 0")` | a fixed-width literal with exact interior spacing |
| **`FORBIDDEN`** (`qemu_run.rs:344`) | `"TEST:FAIL"` | the real message is `TEST:M7:FAIL (init did not exit cleanly)` — the literal is not a substring of it |
| **`errata-limitations`** | `!limitations_src.contains(id)` | anything but the ID string; a divergent *description* passes |
| **`errata-tracking`** | a literal "claims to close" match | did not recognise RFC-0.28-003's phrasing — found while scoping that RFC, in the instrument guarding that very field |
| **`fjell-unsafe-audit`** category extractor | splits on whitespace and commas | `category=csr-asm; …` silently yields `Unknown` |

Seven instruments. **Two are release gates**, and one of those is the gate whose
entire job is to stop a release.

## E-017 — folded in, because it is the same defect one level up

E-017 says the 0.24 audit's `sound` verdicts are not all demonstration-backed.
Counted on 2026-09-09, of 21 `sound` rows:

- **8** have a first-hand demonstration.
- **4** cite `cargo test -p <tool>` — the tool's own unit suite. **This is the
  exact mode-2 proxy attestation E-017 names for Gate 4**, uncorrected for
  Gate 2, Gate 11 and Gate 12.
- **2** inherit another row's demonstration.
- **4** have none at all.
- The register holds **21** such rows; its own summary table says **22**.

**A verdict resting on a proxy and a gate resting on a literal are the same
error** — something stood in for the thing that was supposed to be checked. That
is why these belong in one line rather than two.

## The settled part

**D1 — Parse, do not match.** Every fix decides by structure: parse the table
row and read the status cell, don't grep the rendered line. The template is
RFC-0.28-005's and RFC-0.29-001's — derive the answer instead of pattern-matching
the surface.

**D2 — Gate 7 goes first, and alone if it must.** It is the only one of the
seven that can let a release ship against its own stated rule. If the milestone
runs out of room, Gate 7 is the part that must land.

**D3 — Do not fix a literal with a better literal.** `"| OPEN |"` →
`"| OPEN"` is a wider string, not a predicate. E-014's own text names this: *"each
patch would be a better string."*

**D4 — Every fix demonstrated failing** (RFC-v0.22-001), on the input the old
predicate missed — an annotated `OPEN` for Gate 7, a failed regeneration for
Gate 6, the real `TEST:M7:FAIL (…)` line for `FORBIDDEN`.

**D5 — E-017's four "no demonstration" rows get one produced; the four
"unit suite" rows get their basis corrected to cite the real demonstrations that
now exist** (Gate 2's live category violation, Gate 11's callsite checks from
0.28, Gate 12's ten subchecks from 0.27). Correcting a citation is not the same
as producing evidence, and the register must not blur them.

**D6 — E-015's two surviving lines** — `smoke.rs`'s `v0.6-verification` match arm
and its mention in the usage string — are deleted or wired up, and **E-015
closes.** It is two lines; it does not get its own line.

## The open question — §7

**Should the errata register have one parser?**

Today at least four instruments re-parse `ERRATA.md` independently — Gate 7,
`errata-limitations`, `errata-tracking`, and the summary tally — each with its
own idea of what a row is. Fixing them one at a time yields four parsers that
can disagree, which is E-015's family arriving by way of E-014's fix.

1. **One shared parser**, in `fjell-consistency-check`, that everything reads.
2. **Fix each in place**, accepting four correct-but-separate implementations.
3. **A machine-readable register** — the table generated from structured data,
   so there is nothing to parse.

**Answer in writing before implementing.** I lean to 1 and note that 3 is the
only one that removes the problem rather than centralising it — and that it is
much the largest. Argue it.

## Requirements

**R1 — Gate 7 parses.** Demonstrated on an annotated `OPEN`.
**R2 — Gate 6 checks the regeneration's exit status**, and decides by structure.
**R3 — Gate 5, `FORBIDDEN`, `errata-limitations`, `errata-tracking`, and the
unsafe-audit category extractor**, each demonstrated on the input it missed.
**R4 — E-017: four demonstrations produced, four bases corrected, two
inheritances accepted or re-derived, and the 21-vs-22 count reconciled.**
**R5 — E-015's two lines removed.**
**R6 — E-014, E-015 and E-017 → `CLOSED`.** If any instance survives, **it is
not closed** — name it and leave it open, as RFC-0.29-001 did.

### Non-goals

- Changing what any gate *decides* — only how it decides it. Gate 7 must still
  block on `0 OPEN`; it must simply be able to see one.
- Rewriting `ERRATA.md`'s format, unless §7 chooses shape 3, which escalates.
- E-034, E-035, E-036, E-037, E-038.
- Any kernel, ABI, or service change.

## Risks

**Gate 7 has never fired.** Making it parse correctly may reveal that something
in the register has been miscounted all along, in either direction. That is a
finding, not a setback — and it is the whole reason to do this before an erratum
ever needs to be `OPEN`.

**Seven instruments is a lot of small edits.** The temptation is to do the easy
five and leave Gate 6's exit-status handling, which is the fiddliest. **Gate 6
discarding `sh`'s result is the second-most consequential item here**, because a
trust report that failed to regenerate still passes.
