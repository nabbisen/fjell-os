# Developer Handoff — RFC-0.43-001

**Governing RFC:** [RFC-0.43-001](../../done/RFC-0.43-001-the-solver-pin-is-enforced.md)
**Milestone:** 0.43 · independent of 0.43-002
**Status:** inherited from the governing RFC (Implemented, 0.43.0)
**Audience:** implementation model

---

## 1. Read the RFC first

Six requirements. R5 is the one that decides whether this line is finished: **the demonstration
of the check refusing is the deliverable, not the check.** This gate was green for a whole
milestone while comparing nothing, so a green run is not evidence of anything here.

## 2. The line in the sand

**Do not make the comparison pass by weakening it.** If the installed solver differs from
`TOOLCHAIN.lock`'s `[z3] version`, that is a finding to report — the lock's own rule is that it
moves only behind a recorded proof-review re-run, and that re-run is not part of this line.

**And do not keep `[z3] path`.** R3 is a ruling, not an option. It is right for the AUR layout and
wrong for a hand-install; a field correct on one of two supported routes is fiction. Remove it.

## 3. Yours to argue, not mine to dictate

1. **Where the z3 figure is read from.** R1 says "the prover that is actually going to run". The
   obvious route is the prover's own `--version`. If it turns out z3 answers separately, or the
   version appears in more than one place with different formatting, **say which you chose and
   why** — the one thing that is not acceptable is two sources that could disagree.
2. **What "fail closed on an unrecognised format" should print.** R6 fixes the content; the shape
   is yours. A reader who sees it should not have to read `verus_check.rs` to know what to do.

## 4. Mechanics and traps

- **The prover is a hand-install on this machine:** `~/.local/bin/verus` is a symlink to
  `~/.local/share/verus/verus`. That is exactly why `[z3] path` is wrong — **do not replace it
  with this machine's path either.**
- **`RUSTUP_TOOLCHAIN` must be unset in your shell.** Verus's launcher sets it internally; an
  inherited value makes failures that look like prover bugs.
- **`verus-check` blocks on a Verus version mismatch only under `--release-required`** and warns
  otherwise — *corrected 2026-11-12; this bullet first said it fails closed outright, which
  overstated the existing behaviour.* Either way, if you edit the lock to produce R5's red the tool
  may refuse for the *wrong* reason. Make the two refusals distinguishable, and quote both so a
  reader can tell which fired.
- **Experiment in a scratch worktree, not the main tree** — `.git-exclude/tmp/<name>` — and put a
  `pwd -P` check in the same command as the first edit. A previous session's experiment landed an
  edit in the main tree because the `cd` and the edit were in different commands.
- **Do not install anything**, including a different z3. If the versions do not match, stop.
- `--all-pilot` runs all three targets; `--release-required` is the gate subset. R5 needs only
  enough to show the refusal, so prefer the cheaper invocation and say which you used.

## 5. Evidence the review request must carry

1. **The refusal, quoted**, with the lock pointing at an uninstalled z3 — and the lock shown
   restored afterwards.
2. **The pass, quoted**, at the true pin, so the check is shown to distinguish the two.
3. **The two refusals distinguished** — a Verus mismatch and a z3 mismatch must not print the same
   thing.
4. **The comment at `verus_check.rs:85`, before and after**, so the review can see it no longer
   over-promises. If any clause of it is still unimplemented, show the comment saying so.
5. **The `[z3]` section after R3**, showing `path` gone and nothing replacing it.
6. **One sentence on the ordinary-contributor path**: that `cargo xtask build` and the host tiers
   are unaffected, with whatever you ran to confirm it.

## 6. Stop and escalate if

- The installed z3 does not match the lock. **Do not update the lock to match the machine** — that
  inverts the pin.
- **The z3 binary** does not report a version, or reports one in a form you cannot parse strictly.
  That is a finding about the solver's interface and changes R1.

  > **Corrected 2026-11-12, after this condition misfired.** It first read *"`verus --version` does
  > not report a z3 version at all"*, and `verus --version` carries no solver version — so taken
  > literally it told the implementer to stop where **R1 told them to continue**: R1 says *the
  > bundled z3* answers `--version`, which it does (`Z3 version 4.16.0 - 64 bit`), and §3.1 of this
  > handoff anticipated exactly that case. **The error was mine**; they followed the RFC and §3.1,
  > disclosed the judgement, and were right.
- Making the check fail closed turns any host tier or `release-rehearsal` gate red for a reason
  unrelated to z3.

## 7. Not yours

`rfcs/ERRATA.md` (E-112's closure), `docs/src/releasing/v1-limitations.md` **row 22**, and
`TOOLCHAIN.lock`'s header comment are the architect's. Row 22 currently says the pin is enforced
for the prover and not for the solver; when this lands it becomes false, and **leaving it to me is
correct** — flag it in the review request rather than editing it.

---

## 8. Amendment 1 — the candidate mode (R7, R8), added 2026-11-12

**R1–R6 are accepted and committed (`7c3967e`); this section is the only outstanding work on this
line.** Read Amendment 1 in the RFC first. It exists because of a consequence of R2 that I wrote
and did not foresee: **the line as built makes the next prover upgrade impossible.**

A new prover bundles a new z3, so at the next upgrade no invocation can run the proofs until
`[z3] version` is edited — while the lock's rule is that it moves *only* behind a recorded
proof-review re-run, which requires running them. The ways through today are editing a copy of the
lock in a scratch worktree, which is what 0.42-004 actually did, or a `VERUS_Z3_PATH` wrapper that
lies. **A procedure performable only by a workaround outside the architecture is what R7 removes.**

**The line in the sand:** the candidate mode must **never** emit `MACHINE-CHECKED-PASS`. Gate 10
counts that exact string (`release_rehearsal.rs:302`) and CI retains `verus-markers.txt` as an
artefact, so a candidate run that emitted it would leave a certified-looking marker in the record
with nothing to distinguish it. The vocabulary already separates `CONFORMANCE-ONLY`; a candidate
state belongs beside it.

**Yours to argue:** the mode's name, the marker's name, and whether R8 (the Verus comparison
blocking in every mode once R7 exists) wants its own commit. I think one commit; argue it if the
diff reads better split.

**Evidence the review request must carry:**
1. A candidate run with **both** pins mismatched, quoted, showing the proofs actually run and the
   distinct marker emitted — and `MACHINE-CHECKED-PASS` **absent** from that output.
2. `--release-required` **refusing** the candidate mode, quoted.
3. Gate 10 shown unaffected: `release-rehearsal`'s Verus gate still reads only real passes. A
   marker log from a candidate run fed through it, if you can arrange that cheaply, is the control
   I would most like to see.
4. R8's behaviour change: a Verus mismatch now blocking outside `--release-required`, with the
   before and after.
5. The 0.42-004 workflow rehearsed end to end on a *copy* of the lock: candidate run → proofs pass
   → pin moved → ordinary run green. That is the procedure R7 exists to make possible, so it is the
   thing to demonstrate.

**Stop and escalate if** making the Verus comparison block in every mode (R8) turns any existing
tier or gate red for a reason unrelated to a version mismatch.
