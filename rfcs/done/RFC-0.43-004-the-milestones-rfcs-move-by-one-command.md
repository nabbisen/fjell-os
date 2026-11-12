# RFC-0.43-004: The milestone's RFCs move by one command

**Status:** Implemented (0.43.0) — accepted 2026-11-12
**Milestone:** 0.43
**Tracks.** **E-116** — its one ungated residue: a wholly skipped `accepted/` → `done/` move.
**Touches** *(indicative)*: `crates/fjell-tools/src/main.rs`, a new module beside
`evidence.rs`, `docs/src/releasing/v0-release-cycle.md`.

---

## What is wrong

Step 7 of the cut is **five tasks performed by hand** — the `git mv`, each RFC's `Status:` line,
each handoff's governing-RFC link, each handoff's own inherited `Status:`, and `rfcs/README.md`'s
two tables and two counts. It was performed correctly at twelve consecutive cuts and **documented
for the first time on 2026-11-12** (E-116), which is the whole reason it is worth a tool: twelve
correct repetitions of an unwritten five-part ritual is luck accumulating, not a process.

**And the realistic failure is not the one E-116 named.** E-116's ungated residue is a *wholly*
skipped move, which has never happened. What happened, in this milestone, is a **partial** one:
`a130ace` moved three RFCs to `done/` and committed **none** of their `Status:` edits.
`handoff-status` caught it — but only after a red commit was already in history, and the check its
author used to verify staging (`git status --short | grep -av '^[MARD]'`) **deleted the one line it
existed to find**, because an unstaged edit to a staged rename is reported `RM` and `RM` starts
with `R`.

**The cause is mechanical and worth naming precisely:** `git mv` stages the rename at the moment it
runs, and the edits come after. Any sequence that moves first and edits second has a window in
which four of five tasks are done and the index already looks plausible. **A tool closes the
window; a gate only reports that it was open.**

## The risk that shapes the design

This command runs **once per milestone**, so it is cold every time it is used, and its author will
not be the person running it. **Its refusals are its main surface**, not its happy path — the same
reasoning `evidence promote` already embodies, whose code refuses a promotion *"rather than
recording"* an unobserved result.

And it mutates a lifecycle folder that is the project's source of truth: *"the folder is the source
of truth"* is why `rfc-status-folder` exists. A tool that half-moves is worse than a hand that
half-moves, because it will be trusted.

## Requirements

**R1 — `cargo xtask rfc promote <version>`** performs all five tasks for **every** RFC in
`rfcs/accepted/` whose id matches `RFC-<version>-NNN`, and reports what it changed.

**R2 — All-or-nothing.** If any one task cannot be completed for any RFC in the set, **nothing is
changed on disk**. Compute the whole edit, then apply it, or refuse.

**R3 — It refuses, naming the reason and changing nothing, when:**
- `accepted/` holds no RFC matching `<version>`;
- an RFC's line-3 `Status:` is not the expected `Accepted — accepted <date>` shape;
- a handoff directory or its `implementation-handoff.md` is missing for an RFC being moved;
- `rfcs/README.md`'s Accepted table has no row for an RFC being moved, or its Implemented table
  cannot be located;
- the five counts in `rfcs/README.md` **already** disagree with the folders before it starts — it
  will not build on a tree that `doc-counts` would fail.

**R4 — It does not touch the git index.** A plain filesystem rename, **not `git mv`**, and it
prints the complete list of changed paths so the caller stages them explicitly. This is the
requirement that removes `a130ace`'s window: with nothing staged early, there is no state in which
the index shows a rename whose edits are missing.

**R5 — `--dry-run`** prints exactly what it would change and touches nothing.

**R6 — The demonstrations are the deliverable.** Each refusal in R3 shown firing, quoted, on a
scratch copy; and the happy path shown on a scratch copy of a real milestone's tree, ending in
`consistency-check --all` passing — which is the thing the five tasks exist to satisfy.

**R7 — The procedure keeps the five tasks written down.** `v0-release-cycle.md`'s step-7 section
calls the command, and **still lists what it does**, so a reader learns the shape of the move and
can perform it by hand if the tool is unavailable. A procedure that says only "run the tool" moves
the knowledge into the tool and loses it.

### Non-goals

- **Committing, or staging.** R4 is deliberate.
- **Running the gates.** The command changes the tree; the caller runs `consistency-check`.
- **Repairing a partial move already in the tree.** Refuse and say what is inconsistent. A repair
  tool would need to guess intent, and the hand-fix is one commit.
- **Any other lifecycle transition** (`proposed/` → `accepted/`, `done/` → `archive/`). If the
  shape generalises, say so; do not build it.

## Risks

- **Rewriting `rfcs/README.md` by text manipulation is the fragile part.** Prefer refusing on an
  unexpected shape to a clever match. The Accepted section's prose paragraph is written fresh each
  milestone and is **not** the tool's business — moving table rows and updating counts is.
- **It will be cold.** Every message it prints should make sense to someone who has not read this
  RFC.
- **Twelve correct hand-performances mean the tool has a high bar**: if it is less reliable than
  the ritual it replaces, it is a regression. That is what R2 and R6 are for.
