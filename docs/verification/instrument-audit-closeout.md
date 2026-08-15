# Instrument Audit — Close-Out and Disposition

**Governing RFC:** [RFC-0.24-001](../../rfcs/done/RFC-0.24-001-instrument-audit.md)
**Register:** [instrument-audit.md](./instrument-audit.md) — the authoritative
row-level record; this document disposes of what it found.
**Author:** architect
**Date:** 2026-08-03

---

## 1. What the audit was, and what it produced

RFC-0.24-001 asked one question of every verification instrument in the
repository, about 55 of them, four passes:

> *What would make this instrument report success without having checked?*

The motivating fact was that **eleven** such instances had been found before the
audit, and every one of them **incidentally** — while doing something else.
Eleven for eleven, by accident. The audit existed to replace luck with a method.

**Result, after all four passes and the RFC-0.24-002 review:**

| | Count |
|---|---|
| Instruments examined | **58** |
| Sound | **22** |
| Findings | **33** |
| `UNAUDITED` | **3** |

Thirty-three findings against twenty-two sound. Before this line, **all of
them were reporting green** — and the sound count is **provisional**, for the
reason in §4.1 and erratum **E-017**.

### The population moved, and that is a result too

The audit was scoped at "55 instruments." It closes at **58**. The three extras
are all components inside instruments that make their own claims, surfaced only
when the tools containing them were repaired: the `fjell-unsafe-audit` category
extractor, and — during RFC-0.24-003 — `fjell-abi-snapshot`'s `pub unsafe fn`
blindness and its lack of impl scope. The boundary moved three times in one
milestone.

"55" was always an enumeration at a granularity someone chose while scoping, not
a measurement. **Expect it to keep moving.** A fixed denominator would be the
more comfortable record and the less honest one.

## 2. Repaired

**RFC-0.24-002** — seven slices, all landed and reviewed:

| Instrument | Was |
|---|---|
| Gate 1 — host test suite | Passed on a workspace that did not compile |
| `smoke.rs` milestone | Unknown milestone silently ran `m8` |
| `unsafe-audit --check` | Computed, printed, and ignored its category verdict |
| `ci-unsafe-audit` | Scoped `--root crates`, missing sites outside it |
| `negative` `lease`/`evidence` | Passed against an empty expectation set |
| `abi/snapshot.json` parser | Reformatted JSON read as zero items |
| `ci-proptest` | Ran **zero** tests under a job named "Property tests" |
| `ci-schema-gate` | Name and comments claimed three behaviours it lacked |

**RFC-0.24-003** — **complete, shipped in 0.24.0**: the ABI snapshot's diff
identity and scanner. 45 items never compared, 15 misnamed, 162 mis-attributed,
17 phantom — plus two found during the repair itself: **two `pub unsafe fn`
items in the syscall-ABI crate that had never appeared in any snapshot at all**,
and methods lacking impl scope, caught by the new duplicate-key check on its
first run. Gate 4 is now the first row in this audit to reach `sound` on a live
demonstration against real committed input rather than a proxy.

## 3. Disposition of the 33 open findings

Grouped by root cause, because they are not 33 independent defects. Row-level
detail stays in the register.

### 3.1 Literal-predicate family → **E-014**, 0.25 candidate

Instruments that decide a property by matching a fixed string.

- **Gate 5** counts rows containing `**OPEN**`; a row marked `**BLOCKED**` is
  invisible to every bucket — not miscounted, absent.
- **Gate 6** counts the literals `§1`..`§6` and discards the regeneration's own
  exit status.
- **Gate 7** counts `OPEN` in the errata register.
- **`FORBIDDEN`** looks for `"TEST:FAIL"`, which does not match the real message
  `TEST:M7:FAIL (init did not exit cleanly)`.
- **`errata-limitations`** requires only that an erratum's *ID* appear in
  `v1-limitations.md` — it passed over a live content divergence that the
  architect introduced and the implementer found.
- **`fjell-unsafe-audit`'s category extractor** splits on whitespace and commas,
  so `category=csr-asm; …` silently yields `Unknown`.

**Why deferred rather than patched:** each patch would be a better string. The
family needs one answer to *how these instruments should decide*, and that is
design work, not seven edits.

### 3.2 Explicit-list and matrix staleness → **E-015**, 0.25 candidate

Instruments that enumerate their subjects by hand and drift from reality.

- **19 of 89 workspace crates are never named in `ci.yml`.** Six are the gate
  tools; three back Gate 8's validation drills.
- **`ci-qemu-negative`'s matrix lists nine categories; `test-all` runs ten** —
  `semantic`, added by RFC-v0.23-001, has never run in ordinary CI.
- **`KNOWN_V01X_CATEGORIES` / `KNOWN_V02_CATEGORIES`** no longer describe the
  profiles on disk.
- **`smoke.rs`'s `v0.6-verification`** is defined in code and invoked by
  nothing, anywhere.

**Note the shape difference from 3.1.** These are checks that **do not run**, or
run over an incomplete set — not checks that lie. That distinction is what kept
them out of the pre-cut repair line, and it is the right line to hold.

### 3.3 No link-or-count integrity instrument → **E-016**, 0.25 candidate

Three findings, one missing instrument.

- **`rfcs/README.md` has zero instrument coverage.** The only trace of the
  repository's own RFC index anywhere in the instrument set is a **doc comment
  in `rfc_status_folder.rs` that mentions the file without opening it.** A search
  for coverage found a sentence claiming coverage.
- **13 broken relative links** in tracked documentation.
- **The index's "Shipped" column names a release for ~150 rows as `v0.3.0`,
  `v0.22.0`, and so on — tags that do not exist under those names.** `git tag`
  has never carried a `v`. Its section headers do the same.

The drift and the reason nobody noticed are the same finding. One
link-and-count checker closes all three, and **adding an instrument was
RFC-0.24-001's explicit non-goal** — which is why this waits for 0.25 rather
than being fixed quietly by the person who would then write the checker.

### 3.4 E-013's second confirmation → **E-013 widened**

The audit found the gate tools' own tests unreachable via `test-all` tier 1
(`--lib`, no lib target). Pass 4 found the same six crates are **also never named
in any CI job.** Nothing runs them, anywhere, by any mechanism, in ordinary
operation.

Not a new erratum — the same one, confirmed by a second independent mechanism,
and the disclosure understates without it.

**The three drill crates are deliberately *not* folded in.** Their tests exist
and are reachable; CI simply never invokes them. That is 3.2, and filing it
under E-013 would blur an erratum that is currently precise.

### 3.5 `release-rehearsal` has no proptest gate

Twenty-four property tests — including the 14 `verus_lemma_properties` cases
that cross-check the proofs behind capability 8/8 and lease 5/5 — run in **no
release-time instrument**. `ci-proptest` now runs them on push (Slice 6), and
`test-all` tier 2 runs them manually. Neither is the release gate.

Adding one is **adding an instrument**. 0.25 candidate, and the most
release-relevant of them.

### 3.6 `storaged.rs` is dead code

`crates/fjell-service-api/src/storaged.rs` has no `mod` declaration anywhere;
`lib.rs` declares `pub mod storaged { … }` inline instead. RFC-0.24-003 stops
scanning it. **Whether to delete it is a source-hygiene question for its owner,
not an instrument repair.** Recorded, not actioned.

### 3.7 The three `UNAUDITED`

- **Gate 9** — release-notes limitations, manual by design.
- **`ci-docs`** — mdbook's own link-checking behaviour is a third-party question,
  not this repository's instrument.
- **`ci-fuzz-nightly`** — schedule-only; a short manufactured run would not
  honestly answer the claim.

**These stay `UNAUDITED` and are not converted to anything.** The distinction
between "no evidence" and "evidence of soundness" is the audit's whole integrity,
and quietly promoting the three inconvenient rows at close-out would be the
neatest possible way to lose it.

## 4. The finding about the method

### 4.1 A `sound` verdict is a claim, and claims need demonstrations

Two `sound` verdicts were reversed in review:

- **`ci-proptest`** (Pass 4) — certified on the completeness of its crate list.
  The list was right; the predicate (`--lib`) was never examined. It ran zero
  tests.
- **Gate 4** (Pass 1, **architect-approved**) — certified because the tool's own
  unit suite passed. That is not the gate observed failing on a broken repository
  state; it is **mode 2, proxy attestation**, the taxonomy's own second entry.

Both are the same error: something *adjacent to* a demonstration accepted *as*
one. RFC-0.24-001 §0.1 already forbade it — *"an instrument with no
demonstration is recorded `UNAUDITED`, never `sound`"* — and it happened twice
anyway, once on each side of the review boundary.

**Carried forward: all 18 `sound` rows are re-derived against one question —
*was a demonstration produced, or was something else mistaken for one?*** Gate 4
was the first attempted and it fell immediately. **This work is not complete**,
and the sound count should be read as provisional until it is.

### 4.2 The audit was partly an audit of the review process, as intended

RFC-0.24-001 said so up front: most of the eleven prior instances passed through
architect review without being caught. The line bore that out — errors on both
sides, found by the other:

| Found by | What |
|---|---|
| Implementer | Architect's `grep "^publish = false"` premise (whitespace-brittle, ten false negatives) |
| Implementer | Architect widened E-013 in `ERRATA.md` and not in `v1-limitations.md` |
| Architect | `ci-proptest` certified sound while running zero tests |
| Architect | Gate 4 — his own Pass 1 verdict — resting on proxy attestation |
| Architect | 21/91 crate count against a population including two non-members |
| Architect | Own sizing of the ABI repair, wrong by three defects, after recommending it to the owner |

The pattern is that **neither side caught its own**. That is an argument for the
review boundary, not against either party — and it is the strongest practical
result of the line.

### 4.3 A rule the line produced

Stated when it was needed, recorded because it will recur:

> **A finding discovered while certifying an instrument sound cannot be deferred
> past that certification.** Either the instrument is not certified, or the
> finding is repaired.

This is why 33 findings legitimately defer — none blocks a certification — and
why Gate 4's could not.

### 4.4 Scope discipline, honestly reported

The pre-cut repair group was declared final at four, then seven, then an eighth
item became its own RFC. Two of those three extensions were justified by new
severe findings; **the framing "this is final" was wrong each time it was
written.** Recorded plainly rather than smoothed over, because RFC-0.24-001's R2
predicted exactly this and the prediction is more useful than the reassurance.

## 5. Errata filed

Per the Pass 2 ruling — findings still unfixed at the audit's close get errata,
grouped by family rather than one per finding, so the register is not inflated
with items being fixed inside the same line.

| Erratum | Covers | Status |
|---|---|---|
| **E-013** *(widened)* | Gate tools' tests unreachable in CI as well as tier 1 | ACCEPTED |
| **E-014** | Literal-predicate family (§3.1) | ACCEPTED |
| **E-015** | Explicit-list and matrix staleness (§3.2) | ACCEPTED |
| **E-016** | No link-or-count integrity instrument (§3.3) | ACCEPTED |

`ERRATA.md` and `docs/release/v1-limitations.md` were updated **in the same
edit**. Splitting them is what produced the live divergence the audit found;
`errata-limitations` matches only the ID and would not have caught it a second
time either.

## 6. 0.25 candidates

In the order I would sequence them:

1. **A `release-rehearsal` proptest gate** (§3.5) — most release-relevant.
2. **The literal-predicate answer** (§3.1, E-014) — the largest family, and
   design work rather than patching.
3. **A link-and-count integrity instrument** (§3.3, E-016) — closes three
   findings including the index's zero coverage.
4. **CI list and matrix reconciliation** (§3.2, E-015) — mechanical once
   someone decides whether CI should enumerate or derive.
5. **E-013's fix** — nine host binaries are trivial; `fjell-kernel` is
   architectural and its own RFC.
6. **Completing §4.1's re-derivation of all 18 `sound` rows.**

Not a plan. The 0.25 direction is the owner's.

## 7. What this milestone should say when it is cut

0.24 was scoped as an audit and became an audit, then a repair line, then a
correctness line in the tool that guards the ABI — each step justified by the
previous one finding something real.

The release record should say plainly: **this milestone made the instruments
more honest, not honest.** Thirty-three findings remain open, their instruments
are still green, and they are recorded, grouped, disclosed, and scheduled rather
than fixed.

That is a better position than the one this line started from, where all 58 were
green and nobody had asked why.
