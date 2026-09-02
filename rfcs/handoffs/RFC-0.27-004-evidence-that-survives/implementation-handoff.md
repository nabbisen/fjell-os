# Developer Handoff — RFC-0.27-004

**Governing RFC:** [RFC-0.27-004](../../accepted/RFC-0.27-004-evidence-that-survives.md)
**Milestone:** 0.27
**Status:** inherited from the governing RFC (Accepted, 2026-08-31)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The temptation here is to produce evidence

R6 asks you to reconcile four existing citations. Some of them point at logs
that are gone, produced by instrumented builds that no longer exist. **You will
be able to re-run something close and produce a log that looks right.**

Do not. **D4 is absolute: annotate, never re-manufacture.** A log presented as
the original when it was regenerated later, from different source, is a forgery
in the project's evidence chain — and it is undetectable afterwards, which is
precisely what makes it worse than the gap it would paper over.

The honest output of R6 is a **count**: how many historical citations could be
resolved and how many could not. That number is what E-026 cost, and it belongs
in the erratum when it closes. A low count is not a better result than a high
one; an accurate count is.

## 0.1 The one that is easy to miss — D3

Provenance that records run id and commit sha looks complete and is not.

RFC-0.27-002's accepted evidence came from a build carrying temporary
diagnostics, removed before submission. That log records real behaviour and
**cannot be reproduced from the commit it names.** A future reader who checks
out that sha, re-runs, and sees none of the `DIAG:` lines will reasonably
conclude the evidence was fabricated.

So provenance must state **whether the build was instrumented, what the
instrumentation was, and that it is gone.** A log that silently cannot be
reproduced is worse than one that says it cannot, and this is the field nobody
thinks to add.

## 0.2 Design decisions settled — do not re-open

1. **Promotion is deliberate** (D1). Never a side effect of `test-all`. If
   running the suite can populate `tests/evidence/`, the design is wrong.
2. **Provenance is mandatory** (D2/D3) — run id, commit sha, profile/tier,
   command, instrumented yes/no.
3. **Historical gaps are annotated** (D4).
4. **This does not make claims true** (D5) — disclose in the tooling, the
   evidence directory's README, and the release cycle. Three places, as
   RFC-0.27-003 did.

---

## 1. Order

**R3 → R1 → R2 → §7 answered → R4 → R5 → R6.**

R3 first: until `qemu-run` stops letting the next run destroy the previous
log, there is nothing to promote and you will be testing R2 against files that
vanish underneath it. `crates/fjell-tools/src/qemu_run.rs:178` is the site.

R6 last, deliberately — reconciling the historical citations is the part most
likely to tempt you into producing a log, and doing it after the machinery
exists means the honest path is also the easy one.

## 2. §7 is a real question

**Does an unresolvable historical citation block a release?** Three shapes in
the RFC, including a sunset. I have not stated an inclination this time; the
last three lines each overturned one of mine, and this question is genuinely
open. Argue it.

## 3. R4 — direction B is the requirement, not the nicety

Direction A (every citation resolves) is obvious. **Direction B — every
evidence file is cited by something — is what stops `tests/evidence/` becoming
the landfill that `tests/runs/` already is.** An orphan means either a document
was deleted without its evidence or a log was promoted for no reason. Both are
drift.

## 4. R5 — four demonstrations, and the fourth is the point

| # | Broken input | Must |
|---|---|---|
| 1 | citation to a missing evidence file | FAIL, naming both |
| 2 | evidence file with no provenance | FAIL, naming the file |
| 3 | orphan evidence file | FAIL, naming it |
| 4 | **provenance naming a commit sha that is not an ancestor of HEAD** | FAIL |

4 is what a hand-edited or copied-in log looks like. If you conclude it cannot
be checked cheaply — shallow clones, or a sha from an unmerged branch — **say so
in writing and propose what you would check instead.** Do not drop it silently.

## 5. Prohibited shortcuts

- Do not re-run anything to stand in for a lost log (D4).
- Do not let `test-all` or `release-rehearsal` promote evidence (D1).
- Do not widen the `.gitignore` exception beyond `tests/evidence/`.
- Do not make `fjell-kernel` host-testable — that is E-013, not this line.
- Do not claim a committed log proves the claim it evidences (D5).
- Do not touch the kernel, the ABI, the syscall surface, or any service.
- Do not run `cargo fmt --all --check` in your head.

## 6. Required evidence

1. `tests/evidence/` with a README carrying D5's disclosure.
2. The narrow `.gitignore` exception, and a demonstration that a routine
   `test-all` run lands nothing in it.
3. `qemu-run` retaining per-run logs (R3).
4. The promotion command (R2), with provenance including the D3 field.
5. **§7 answered in writing**, with the two rejected shapes.
6. The subcheck, both directions, with unit tests. Gate 12 goes **9 → 10**;
   update its label in the same commit — it has been wrong once before.
7. **All four demonstrations, captured.**
8. R6's reconciliation, **with the resolved/unresolvable count stated**.
9. `release-rehearsal` green; `test-all` 21/21; `syscall-surface` **35/29/6**.
10. `cargo fmt --all --check` — run it.

## 7. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **The unresolvable count from R6**, and for each one, what it claimed and why
  it cannot be checked.
- **Your §7 answer**, and which parts are observation versus judgement.
- Any citation you were tempted to resolve by re-running, and what stopped you.
- Anything you found while reading that is not in this RFC. Every line this
  milestone has turned up at least one, and two of them changed the RFC that
  followed.
