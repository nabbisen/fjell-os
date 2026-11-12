# RFC-0.43-001: The solver pin is enforced, and the comment stops describing what the code does not do

**Status:** Implemented (0.43.0) — accepted 2026-11-12
**Milestone:** 0.43
**Tracks.** **E-112** — `TOOLCHAIN.lock` pins the SMT solver and nothing checks it.
**Touches** *(indicative)*: `crates/fjell-tools/src/verus_check.rs`,
`verification/verus/TOOLCHAIN.lock`, `verification/verus/TOOLCHAIN.md`.

---

## What is wrong

`verification/verus/TOOLCHAIN.lock` opens by calling itself a **reproducibility pin** whose rule
is *"Update only with a recorded proof-review re-run"*, and it carries a `[z3]` section with a
`version` and a `path`. `verus_check.rs:85`'s own comment says the check will *"print detected
Verus/z3 versions and compare against the pinned `TOOLCHAIN.lock` release"*.

It does neither for z3. It calls `detect_verus_version()` and `read_lock_release()`, compares the
**Verus release only**, and never detects, prints or compares z3. So the z3 pin is
**documentation, not enforcement**, and the comment describes behaviour the code does not have.

**This was not hypothetical.** Across 0.42-004 the bundled solver moved **4.12.5 → 4.16.0** and no
gate noticed. All 20 obligations re-verified unchanged, which was **luck rather than process**: had
one failed, the cause would have been ambiguous between two simultaneous changes, because nothing
recorded which solver ran.

## The risk that shapes the design

**z3 is what actually discharges the obligations.** Verus translates; the solver decides. A solver
change is the likeliest single cause of a proof ceasing to verify — different heuristics, different
incompleteness, different timeouts — so the component with the greater effect on whether
`20 verified, 0 errors` means anything is the one with no gate behind it.

**The second risk is the surface, and it carries equal weight.** This check fails closed on a
release-blocking gate. A contributor whose bundled z3 differs will be stopped by it, and most
contributors **need Verus for nothing at all** — `TOOLCHAIN.md` states it is never a Fjell build
dependency. A refusal that does not say which component mismatched, what was expected, what was
detected, and that an ordinary build is unaffected is a defect in this line, not a polish item.

## Requirements

**R1 — Detect the solver from the prover that is actually going to run.** The bundled z3 answers
`--version`. Derive the figure from the installed prover, not from a path recorded in the lock.

**R2 — Compare it against the lock's `[z3] version`, and fail closed in every mode.** A mismatch
stops the run; it is not a warning.

> **Corrected 2026-11-12, during review.** This requirement first read *"fail closed, exactly as
> the prover comparison already does"* **and** *"a mismatch stops the run; it is not a warning"*.
> Those disagree: the prover comparison warns outside `--release-required` and blocks only under
> it. The implementer took the second sentence and said so, which was right — Gate 10 counts
> `:MACHINE-CHECKED-PASS` out of a marker log CI retains, so a certified-looking marker produced
> under an unrecorded solver must not be reachable in any mode. The requirement now says only
> that.

**R3 — Delete the `[z3] path` field.** `path = "/opt/verus/z3"` is the AUR layout and is wrong for
a hand-install, where z3 sits beside the binary — and a field that is correct on one of two
supported install routes is fiction. The authority on what ran is the prover's own report, which
R1 makes the check read. **Do not "make the path accurate": remove it.**

**R4 — The comment must describe the code.** `verus_check.rs:85` currently promises more than the
code delivers. When R1 and R2 land it becomes true; it must not be left standing in any state
where it over-promises, and if any part of its claim is still not implemented, the comment says so
in the comment rather than in a handoff.

**R5 — The failing-first demonstration is the deliverable, not the feature.** Point the lock at a
z3 version that is not installed, run `verus-check`, and quote the refusal. Then restore the lock
and quote the pass. **An instrument claimed sound carries a committed demonstration of it
failing** — RFC-0.24-001's rule, the one **E-017** was filed over. A green run proves nothing here:
the check was *already* green while comparing nothing.

**R6 — The refusal names the component, both versions, and the blast radius.** Expected versus
detected, which of the two pins mismatched, and one sentence that Verus is not required for an
ordinary `cargo xtask build` or any host tier.

### Non-goals

- **Pinning z3 independently of Verus.** It is bundled with the prover; the lock records what the
  prover carries, not a separately installed solver. Do not add a second install route.
- **Re-running the pilot proofs.** The pin does not move in this line. If a re-run happens for
  other reasons, it is not evidence for this line's requirements.
- **Installing anything.** If the detected solver does not match, that is the finding.

## Risks

- **The `--version` string's format may vary between releases.** Parse it strictly and **fail
  closed on a format you do not recognise** — a parser that falls back to "assume it matches"
  reproduces the defect this line exists to remove. Report the exact string you found.
- **Do not reach for a looser comparison to make the check pass.** If the installed solver
  genuinely differs from the lock, the answer is a recorded proof-review re-run under
  `TOOLCHAIN.lock`'s own rule, not a weaker predicate.
- **The check must not become a reason to skip `verus-check`.** If it proves noisy in practice,
  report that rather than relaxing it; an instrument worked around by rephrasing is worth less than
  none.

---

## Amendment 1 — a candidate mode, because the line as built makes the next upgrade impossible

**Added 2026-11-12 by the architect, at this line's review. This is a consequence of R2, which I
wrote, and not a defect in the implementation.**

The z3 comparison blocks in every mode with no escape hatch. A new prover bundles a new z3, so at
the next upgrade **no invocation of `verus-check` can run the proofs** until `[z3] version` is
edited — while `TOOLCHAIN.lock`'s rule is *"Update only with a recorded proof-review re-run"*,
which requires running them. Circular. The only ways through are editing a copy of the lock in a
scratch worktree — what 0.42-004 actually did — or a `VERUS_Z3_PATH` wrapper that lies. **A
procedure performable only by a workaround outside the architecture fails the design bar's first
clause**, and accepting the line as it stands would institutionalise that workaround.

**R7 — a named candidate mode.** `verus-check --candidate` (name is the implementer's to choose)
runs the proofs with either pin mismatched, for the express purpose of a recorded re-run, and:

- **emits a distinct marker and never `MACHINE-CHECKED-PASS`.** Gate 10 counts that exact string
  and CI retains `verus-markers.txt` as an artefact; the marker vocabulary already separates
  `CONFORMANCE-ONLY`, so a candidate state fits the existing shape rather than inventing one.
- **is refused under `--release-required`**, so it can never stand in for certification.
- **states in its own output that the run certifies nothing**, naming both detected versions and
  both pins.

**R8 — the Verus release comparison blocks in every mode too, once R7 exists.** The asymmetry the
implementer flagged — z3 blocking always, the prover warning outside `--release-required` — is
real and should not survive. The reason the prover comparison only warned was to leave room for
precisely the experiment R7 now provides properly. Two pins, one behaviour, and the room to
experiment named as a mode rather than hidden in a severity.

**Not required, and explicitly out of scope:** making the candidate mode usable in CI, or recording
candidate runs anywhere a gate reads. A candidate run is a human step inside a recorded re-run, and
`TOOLCHAIN.lock`'s procedure already says what that record is.
