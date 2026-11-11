# RFC-0.43-001: The solver pin is enforced, and the comment stops describing what the code does not do

**Status:** Accepted — accepted 2026-11-12
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

**R2 — Compare it against the lock's `[z3] version`, and fail closed**, exactly as the prover
comparison already does. A mismatch stops the run; it is not a warning.

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
