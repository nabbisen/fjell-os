# RFC-0.35-003: Instruments that answer the question you asked

**Status:** Implemented (0.35.0) — accepted 2026-10-14
**Milestone:** 0.35
**Tracks.** **E-071** — `fjell-repro-check` silently ignores an unrecognised flag
and falls through to its full two-build mode, and has no `--help`, so the natural
way to ask for usage starts a build that can race a live `test-all` — and **the
readiness-summary gap**, which has no erratum because it was found and corrected by
hand twice and nothing checks it: `docs/src/releasing/v1-readiness.md`'s summary is
not compared with the rows it summarises. One line because they are one defect:
**an instrument that reports on less than it claims**, once in a tool's argument
handling and once in the document that is the v1.0 gate.
**Touches** *(indicative)*: `crates/fjell-tools/src/` (`fjell-repro-check`),
`tools/fjell-consistency-check/src/`, `docs/src/releasing/v1-readiness.md`,
`rfcs/ERRATA.md`, `docs/src/releasing/v1-limitations.md`.
**Relates to:** **E-014**, the class both belong to; **trap 7** of
`docs/src/releasing/release-handoff.md` — *one `test-all` at a time per tree* —
which an accidental build violates; **Gate 5**, which passes on the readiness
matrix while never comparing its claims with its contents.

## Summary

Derived 2026-10-14.

**E-071.** The tool reads flags with `args.iter().any(|a| a == "--skip-build")`.
Any unrecognised argument is ignored and the tool proceeds to its full two-build
mode, rebuilding into the shared `target/`. There is **no `--help`**, so asking the
tool what its flags are is the way to start the expensive path. **This has fired
twice** — once after a prebuilt rebuild during 0.33, and again at RFC-0.34-004,
where `fjell-repro-check -- --help` began a real two-build run beside a live
`test-all`, killed by a broken pipe before any write completed.

**The readiness summary.** `v1-readiness.md`'s *Summary at …* block states a count
per status. Until 2026-10-13 it disagreed with its own rows in three ways at once —
headed *"at v0.9.4"*, claiming **DONE 42** where `readiness-check` counts **56**,
and saying **three** IN PROGRESS where the table said **0** and the rows numbered
**2** — and **Gate 5 passed throughout**, because it looks only for `**OPEN**`
cells. On 2026-10-14 the same block had to be amended again: its heading still said
0.33.0 after 0.34.0 shipped, and the correction note itself had gone stale, having
promised that 0.34 would carry this very check. **Two hand corrections in two days,
in the document that defines what v1.0 requires, with a green gate beside them.**

That is the argument for this line. An instrument that cannot catch a document
contradicting itself is not measuring the document.

## Requirements

**R1 — Re-derive** both at your tip, with controls: the flag predicate and the
absent `--help`; and that nothing compares the readiness summary with its rows
(name what Gate 5 *does* check, so the gap is stated rather than implied).

**R2 — `fjell-repro-check` refuses what it does not understand.** An unrecognised
argument **exits non-zero with the usage**. `--help` prints the usage and exits
**zero**. Neither starts a build.

**R3 — The failing case, shown.** A transcript of an unknown flag starting a build
**before** the change and not starting one after. An absence probe is not evidence:
the demonstration is the deliverable, per the register's own closing condition for
this entry.

**R4 — The same predicate audited across the tools.** `args.iter().any(…)` is a
pattern, not an incident. Sweep `crates/fjell-tools/` and `tools/` for argument
handling that ignores the unrecognised, and report what you found — including
"nothing else", if that is the answer, with the control that makes it credible.
Fix what is the same defect; **file, do not fix**, anything that turns out to be a
different one.

**R5 — A subcheck that compares the readiness summary with its rows.** Gate 12's
20th (19th is RFC-0.35-001's): the per-status counts in `v1-readiness.md`'s summary
must equal what `readiness-check` counts, **and** the summary's prose must not
state a figure the table contradicts. It must **fail** against the state of
2026-10-12 — reconstruct that block in a fixture and show the failure — and pass
against the tree. A subcheck first seen passing has tested nothing.

**R6 — The heading is part of the claim.** *"Summary at 0.34.0"* is a factual
assertion about when the counts were taken. Decide whether the check enforces it
against the released version and say which; if it does not, say why, so the next
person does not assume it does.

**R7 — E-071 CLOSED** with its demonstration, `v1-limitations.md` in the same
commit, and the readiness gap recorded — as a closed erratum if you file one, or in
the subcheck's own doc comment, but **somewhere a reader will find it**. The
correction note in `v1-readiness.md` that promised 0.34 would carry this check
should now name the line that did.

**R8 — The gates**, each by its own exit status; `test-all`; a CI run id.

### Non-goals

- Making `fjell-repro-check` cheaper, or changing what either of its modes does.
- A general argument-parsing library for the workspace.
- Deciding whether an IN PROGRESS readiness cell may reach the v1.0 tag — that
  question is explicitly left open by the matrix and stays open.

## Risks

- **A `--help` that is written and never run.** The rule the register already
  states for this entry is that the *failing* case must be shown; a `--help` whose
  output nobody read is the same class of defect in a new place. Run every command
  this line adds to a document before committing it.
- **The new subcheck can be written to pass.** R5's fixture exists to stop that.
  If reconstructing 2026-10-12's block proves awkward, that awkwardness is the
  point — do not substitute a synthetic case that fails for a different reason.
- **Sweeping for a pattern invites scope.** R4 is "report, fix the same defect,
  file the rest". Resist repairing every tool you read.
