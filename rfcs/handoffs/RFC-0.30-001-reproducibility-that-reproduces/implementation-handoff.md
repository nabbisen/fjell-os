# Developer Handoff — RFC-0.30-001

**Governing RFC:** [RFC-0.30-001](../../accepted/RFC-0.30-001-reproducibility-that-reproduces.md)
**Milestone:** 0.30 — the first line of the milestone
**Status:** inherited from the governing RFC (Accepted, 2026-09-09)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. Measure before you design

**R1 comes first and it is not a formality.** Nobody knows what a genuine
two-build run costs, because nobody has done one. §5 asks where the check should
live — CI, the cut, or nightly — and **that question cannot be answered without
the number.**

So: make the two builds real, time them, report the wall clock. Then argue §5.
Designing the placement first and measuring afterwards is how the answer becomes
whatever the design already assumed.

## 0.1 Expect it to fail, and let it

A real two-build comparison may well fail the first time it runs. Embedded
build paths, timestamps, and `-C metadata` are the usual causes, and this
project has already seen `-C metadata` move digests on a version bump.

**A failure is the finding of the milestone.** The instinct to narrow the check
until it passes is the instinct that produced the tautology in the first place —
and this project shipped `ci-proptest` once already, a job named for a check
that ran zero tests.

If the build is genuinely not reproducible, **say so, document it, and correct
T20 to match.** A reproducible build that is honestly documented as not-yet-
reproducible is worth more than a green check that proves nothing.

## 0.2 Design decisions settled — do not re-open

1. **T20's sentence is corrected regardless of how R2 lands** (D1). A threat
   model naming a defence that cannot fail is worse than one naming none.
2. **Two genuinely independent builds** (D2) — separate target dirs, or a clean
   between. And **demonstrated failing** on a deliberately non-reproducible
   input.
3. **The kernel's coverage is settled** (D3): in the baseline, or its absence
   stated in the documents. Not the present state, where one code path collects
   it and the other does not and neither says so.
4. **This is same-machine reproducibility, not cross-machine** (D4). E-037 keeps
   the toolchain unrecorded; do not claim what that prevents.

---

## 1. Order

**R1 (measure) → §5 answered → R2 (real check + demonstration) → R3 (kernel) →
R4 (T20) → close.**

## 2. The counts, to re-derive

I claim: `two_build_check` collects **30** artefacts (29 `prebuilt/*.bin` plus
the kernel ELF); the baseline holds **29**, all prebuilts; and the two builds
completed in **0.41s and 0.40s**, which is what a no-op looks like.

**Re-derive all of it.** Eight consecutive lines have corrected a number I
asserted, and the last two were figures I had put in required evidence. If the
30/29 split is different, or if there is a clean step I missed, say so.

## 3. What `--skip-build` is, and leave it that way

It compares committed prebuilts against a stored baseline. It catches an
artefact rebuilt without re-recording — a real incident this project has had.
**It is a staleness check.** Do not extend it into a reproducibility check; name
it honestly and leave it alone. Two checks doing two things beats one doing
neither well.

## 4. Prohibited shortcuts

- Do not design the placement before measuring the cost.
- Do not narrow the check to get a green result.
- Do not claim cross-machine reproducibility — E-037 is untouched.
- Do not leave T20's sentence as it stands under any outcome.
- Do not touch the kernel source, the ABI, or any service.
- Do not run `cargo fmt --all --check` in your head, and put it in the evidence
  list.

## 5. Required evidence

1. **R1's measurement** — wall-clock cost of a genuine two-build run, stated
   before the §5 argument.
2. **§5 answered in writing**, with the rejected placements and the measured
   cost cited.
3. **The real two-build check, demonstrated failing** on a deliberately
   non-reproducible input — and the input named.
4. **Whether the build is actually reproducible**, answered honestly. If it is
   not, what makes it not.
5. The kernel's coverage settled, and stated wherever the evidence is described.
6. **T20's text corrected**, matching what is now verified.
7. **E-036 `CLOSED`** — including the case where the check proves infeasible, in
   which case a new erratum records the missing capability. Register and
   `v1-limitations.md` in the same commit.
8. `release-rehearsal` green; `test-all` all tiers; `syscall-surface` 35/29/6;
   `callsite-audit` 5 checks.
9. `cargo fmt --all --check`.

## 6. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **Whether the build reproduces**, and if not, exactly what breaks it. This is
  what I will read hardest, and "it passed" is the answer I will scrutinise most.
- R1's number, and how §5 followed from it rather than preceding it.
- The demonstration input you chose, and why it is representative.
- Any count of mine you re-derived and found different.
