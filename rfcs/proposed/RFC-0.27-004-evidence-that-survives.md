# RFC-0.27-004: Evidence that survives the run that produced it

**Status:** Proposed — awaiting owner acceptance
**Milestone:** 0.27 — *the number presumes 0.27; if the owner would rather this
land in 0.28 it renumbers, and nothing else changes.*
**Tracks.** **E-026**: no QEMU serial log this project cites has ever been
committed alongside the document citing it.
**Touches.** `.gitignore`, `tests/evidence/` (new), `crates/fjell-tools`
(`qemu_run`), `tools/fjell-consistency-check`, the release cycle, and the
existing citations in `docs/` and `rfcs/`. **Does not touch the kernel, the ABI,
or any service.**
**Relates to:** **E-013** (why QEMU logs are the only evidence there is);
**E-027** and **E-023** (documents asserting what was never built);
RFC-0.27-001 (the subchecks this extends); RFC-0.27-003 (the same weak-predicate
disclosure applies).

## Summary

Every kernel-side claim this project makes cites a QEMU serial log. **E-013**
guarantees there is no alternative — `fjell-kernel` has no `[lib]`, so nothing
kernel-side is host-testable, and every architect handoff since 0.24 has said
"cite the QEMU log" for exactly that reason.

None of those logs exist.

| Where a log can live | Overwritten by the next run? | In the tree? |
|---|---|---|
| `tests/qemu/artifacts/<profile>/serial.log` | **yes** | no — `.gitignore:28` is `*.log` |
| `tests/runs/<timestamp>/` | no | **no** — also `*.log`, and its per-tier logs hold build stdout, not a serial transcript |

So the standing instruction produces citations that decay by construction. It is
not a mistake anyone made; it is the guaranteed end state of the instruction,
and RFC-0.27-002 is where it became visible: a submission cited
`tests/qemu/artifacts/smoke-m8/serial.log` for a trace that file no longer
contained, and the only surviving copy of the quoted lines was the citing
document itself.

**The claims are not thereby wrong. They are unverifiable from a clone** — a
lesser and different thing, and the thing this line closes.

## Motivation

### Why this is the same defect as the last four milestones, one layer down

0.24 through 0.27 have been one long finding: **documents asserting things
nothing checks.** E-023, an RFC reading `Implemented` with four of five
behaviours unbuilt. E-027, a published document asserting a gate that never
existed. RFC-0.27-003, a mapping whose rows are exactly such assertions, which
is why it ships with an instrument.

E-026 is the layer beneath all of them. Those findings were caught by *reading
the tree*. A kernel claim cannot be caught that way — there is nothing in the
tree to read. **The evidence chain terminates in a file that has been deleted.**

### What this is not

It is **not** an attempt to make `fjell-kernel` host-testable. That is E-013, it
is a much larger line, and this RFC deliberately accepts E-013 and repairs the
chain that E-013 forces the project to depend on.

## The settled part — decisions not to be re-opened

**D1 — Retention is deliberate, never automatic.** Committing every log from
every run of 21 tiers would bury the evidence in noise and make the tree grow
without bound. **Only a log a document actually cites is promoted**, by an
explicit act, into `tests/evidence/`. Everything else stays ignored and
overwritable exactly as today.

**D2 — A committed log without provenance is barely better than no log.** Each
promoted log carries, in a sidecar or a header the tooling writes: the run id,
the **commit sha it was produced from**, the profile or tier, and the exact
command.

**D3 — An instrumented build must say so, on the log.** This is the subtle one
and the reason D2 is not enough. RFC-0.27-002's accepted evidence came from a
build carrying temporary diagnostics that were removed before submission — so
that log **cannot be reproduced from the commit it cites**, and a reader who
tried would conclude the evidence was fabricated. Provenance must record whether
the build was instrumented and, if so, what the instrumentation was and that it
no longer exists. **A log that silently cannot be reproduced is worse than one
that says it cannot.**

**D4 — Historical citations are annotated, never re-manufactured.** Some
existing citations point at logs that are gone and cannot be regenerated,
because the instrumented builds no longer exist. **Do not re-run something
similar and present it as the original.** Mark the citation as unresolvable,
with what it claimed and why it cannot be checked — the E-027 treatment. A
recorded gap is evidence; a re-manufactured log is a forgery.

**D5 — This does not make the claims true.** A committed log proves a run
produced that output. It does not prove the document's reading of it is right —
RFC-0.27-002's first submission read its own trace backwards, and the log would
not have caught that. Same weak predicate as RFC-0.27-003's path check, and it
must be **disclosed in the same three places**: the tooling, the evidence
directory's own README, and the release cycle.

## Requirements

**R1 — `tests/evidence/` and a narrow `.gitignore` exception.** `*.log` stays
ignored; `!tests/evidence/**` un-ignores only this directory. The exception must
be narrow enough that no routine run can land in it by accident.

**R2 — a promotion command**, e.g. `cargo xtask evidence promote`, which copies
a named run's serial log into `tests/evidence/<rfc-id>/<name>.log` and writes
the D2/D3 provenance. Promotion is a deliberate act with an argument, never a
side effect of `test-all`.

**R3 — `qemu-run` retains the serial log per run**, not per profile, so a
promotable copy still exists after the next tier runs. The current
`crates/fjell-tools/src/qemu_run.rs:178` writes `<artifacts>/<profile>/serial.log`
and the next run of that profile destroys it.

**R4 — a subcheck, in both directions.** Like `errata-tracking`:

- **A** — every citation of a `tests/evidence/` path resolves to a file that
  exists and carries provenance.
- **B** — every file in `tests/evidence/` is cited by at least one tracked
  document. An orphan is either a document deleted without its evidence, or
  evidence promoted for no reason; both are drift, and B is what stops the
  directory becoming a landfill.

**R5 — demonstrated failing** per RFC-v0.22-001, on: a citation to a missing
evidence file; an evidence file with no provenance; an orphan; and — the one
that matters — **a provenance block claiming a commit sha that is not an
ancestor of HEAD**, which is what a copied-in or hand-edited log looks like.

**R6 — the existing citations reconciled.** Three tracked documents cite `.log`
paths, plus RFC-0.27-002's answer document. Each is promoted if the log still
exists, or annotated per D4 if it does not. **Report which is which** — the
count of unresolvable historical citations is the honest measure of what E-026
cost, and it belongs in the erratum when it closes.

## The open question — §7

**Does an unresolvable citation block a release?**

RFC-0.27-003 settled the analogous question for the standards mapping as shape
3: only a *vanished* evidence path fails the cut. The parallel answer here is
that annotated historical gaps never block and a broken *new* citation does.

But there is a real argument the other way: a document that cites evidence
nobody can check is precisely what this line exists to end, and grandfathering
it permanently means the tree keeps a class of claim that is exempt. A sunset —
annotated citations tolerated until some milestone, then required to be resolved
or the claim withdrawn — is the third shape.

**Answer in writing before implementing R4**, and say which of the three the
subcheck implements.

## Scope

`.gitignore`; `tests/evidence/`; `crates/fjell-tools` (`qemu_run`, a new
`evidence` subcommand); `tools/fjell-consistency-check`;
`docs/src/release/v0-release-cycle.md`; the four existing citations;
`docs/rfcs/ERRATA.md` (E-026 → `CLOSED`, with R6's count).

### Non-goals

- **Making `fjell-kernel` host-testable** — that is E-013 and a separate line.
- Committing logs wholesale, or retaining every run (D1).
- Re-running anything to stand in for a lost log (D4).
- Verifying that a document's *reading* of a log is correct (D5).
- Any change to the kernel, the ABI, the syscall surface, or any service.
- E-024, E-025, E-027, E-028.

## Risks

**The tree grows.** Serial logs are ~9 KB and the whole artifacts directory is
400 KB, so the size risk is small — but it is unbounded without R4's direction
B, which is why B is a requirement rather than a nicety.

**Committed evidence looks more authoritative than it is.** A reader who finds a
log in the tree will over-trust it, exactly as a green gate is over-trusted.
D5's disclosure is the counterweight and must be written in the project's own
plain language, not buried.
