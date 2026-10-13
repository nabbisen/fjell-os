# Developer Handoff — RFC-0.35-003

**Governing RFC:** [RFC-0.35-003](../../accepted/RFC-0.35-003-instruments-that-answer-the-question-you-asked.md)
**Milestone:** 0.35
**Status:** inherited from the governing RFC (Accepted, 0.35)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design
conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line

**Does asking a tool what its flags are tell you, without starting a build? And
does a gate notice when the v1.0 readiness document contradicts itself?** Both
answers are no today. Both become yes here, each with the case that used to fail
shown failing first.

## 1. Order

1. **R1 re-derivation**, with controls — including naming what Gate 5 actually
   checks, so the gap is stated rather than implied.
2. **R3 first half** — the transcript of an unknown flag starting a build. Take it
   now; after R2 it cannot be taken.
3. **R2** — unknown flag exits non-zero with the usage; `--help` prints it and
   exits zero; neither builds.
4. **R3 second half** — the same flag not starting a build.
5. **R4** — the sweep for the same predicate across `crates/fjell-tools/` and
   `tools/`; report, fix the same defect, **file** anything different.
6. **R5** — the subcheck, written to **fail** against a fixture reconstructing
   2026-10-12's summary block, then passing against the tree.
7. **R6** — the heading decision, stated either way.
8. **R7** — E-071 closed with its demonstration; the readiness gap recorded where a
   reader will find it; the stale promise in `v1-readiness.md`'s correction note
   now naming this line.
9. **R8** — gates, `test-all`, CI.

## 2. Traps specific to this line

1. **Run every command you write into a document, verbatim, on real output,
   before committing it.** This line's whole subject is instruments that were never
   exercised; a `--help` text nobody ran would be the joke writing itself.
2. **Take R3's transcript in a scratch tree**, and check `pwd` in the same command
   as any edit. A two-build run races a live `test-all` over the shared `target/`
   and the same `fjell-disk.img` — that is trap 7, and it is the hazard this line
   exists to remove, so do not reproduce it while removing it. **One `test-all` at
   a time per tree.**
3. **The fixture is the deliverable, not a formality.** R5's subcheck must fail on
   the real historical block, not on a synthetic one that fails for a different
   reason. If the reconstruction is awkward, that is information about the check,
   not an excuse to simplify it.
4. **Two subchecks land this milestone** — RFC-0.35-001's (19th) and this one
   (20th). Gate 12's count changes, and so does every document that states it.
   `doc-counts` and the release record both care.
5. **Do not widen R4.** Reading twelve tools and repairing three of them is how a
   small line becomes a milestone.

## 3. What to put in the review request

- R1's probes and controls, and what Gate 5 checks.
- The unknown flag starting a build, then not.
- `--help`'s actual output, pasted, from a real run.
- R4's sweep: what was found, what was fixed as the same defect, what was filed as
  a different one, and the control behind any "nothing else".
- The new subcheck failing on the fixture and passing on the tree, with Gate 12's
  new subcheck count.
- R6's decision about the heading.
- E-071 closed; where the readiness gap is recorded; the corrected note.
- Each gate's own exit status, `test-all`, a CI run id.

Hand back one path: your review request. Do not tag anything.
