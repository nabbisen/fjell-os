# Developer Handoff — RFC-0.43-004

**Governing RFC:** [RFC-0.43-004](../../done/RFC-0.43-004-the-milestones-rfcs-move-by-one-command.md)
**Milestone:** 0.43 · independent of 0.43-001/002/003, all landed
**Status:** inherited from the governing RFC (Implemented, 0.43.0)
**Audience:** implementation model

---

## 1. Read the RFC first

Seven requirements. **R4 is the one the line exists for** — no `git mv`, no staging — and **R6 is
what finishes it**: each refusal shown firing, not just the happy path. This command runs once per
milestone, so it is cold every time; its refusals are its surface.

## 2. The line in the sand

**Do not use `git mv`, and do not touch the index.** The defect this line removes is a window in
which the rename is staged and the edits are not, which is how `a130ace` committed three renames
with none of their `Status:` edits. Plain rename, then print the changed paths for the caller to
stage explicitly.

**Do not repair a partial move.** If the tree is already half-moved, refuse and say what is
inconsistent. Guessing intent is worse than a one-commit hand fix.

## 3. Yours to argue, not mine to dictate

1. **Where the module lives and what the subcommand is called.** `rfc promote <version>` mirrors
   `evidence promote`; if a different shape reads better in `main.rs`'s dispatch, argue it.
2. **How much of `rfcs/README.md` the tool owns.** The RFC says table rows and counts, not the
   Accepted section's prose paragraph, which is written fresh each milestone. If moving a row
   cleanly requires touching more than that, say what and why before doing it.
3. **Whether `--dry-run` output and the real output should be the same text** with a prefix, or
   different. I have no view; a reader comparing them should not have to guess.

## 4. Mechanics and traps

- **`evidence.rs` is your precedent**, including `cmd_promote` and the comment at line 229 about
  refusing rather than recording. Read it before designing the refusal path.
- **Compute, then apply.** R2 is all-or-nothing, which in practice means building the full set of
  (path, new content) pairs and the rename list first, and writing only if every one was derivable.
- **The five tasks, exactly** — and the fourth is the one `a130ace` missed:
  1. `rfcs/accepted/RFC-<ver>-NNN-*.md` → `rfcs/done/`;
  2. that file's **line 3**, `Status: Accepted — accepted <date>` → `Implemented (<version>) —
     accepted <date>`, **keeping its own date**;
  3. its handoff's **line 3** governing-RFC link, `../../accepted/` → `../../done/`;
  4. its handoff's **line 5**, `Status: inherited from the governing RFC (Accepted, …)` →
     `(Implemented, <version>)`;
  5. `rfcs/README.md`: the row moves from the Accepted table to the Implemented table with
     `<version>` in its `Shipped` column, the Accepted heading becomes `— none` with its one-line
     text when the folder empties, and **both** counts at the top of the file move
     (`## Implemented (done/) — N files`, and the `N RFCs plus \`v0.7.x-index.md\`` line, which is
     that total minus one).
- **The Implemented table's row text is not mechanical.** Each existing row carries a prose summary
  written by hand. The tool moves the row it finds in the Accepted table; it does not write prose.
  If the Accepted row's text is unsuitable as an Implemented row, that is a finding — report it
  rather than inventing a summary.
- **Scratch copies, not the real tree, for every demonstration** — `.git-exclude/tmp/<name>` with a
  `pwd -P` check in the same command as the first edit. A previous session landed an experiment in
  the main tree because the `cd` and the edit were in different commands.
- **There is nothing in `accepted/` to promote right now** except this line's own RFC, which is a
  pleasing hazard: do **not** test by promoting `0.43` in the real tree. Build a scratch milestone,
  or copy a past one.
- `/usr/bin/grep -a` when sweeping, as always here.

## 5. Evidence the review request must carry

1. **Each R3 refusal, quoted**, with the condition that produced it: no matching RFC; a malformed
   `Status:` line; a missing handoff; a missing Accepted-table row; counts already disagreeing.
   Five refusals, five quotes.
2. **The all-or-nothing property shown, not asserted** (R2): a scratch tree where one RFC of two is
   malformed, the command refused, and **`git status` empty afterwards**.
3. **The happy path on a scratch copy of a real milestone**, ending with `consistency-check --all`
   passing on that scratch tree — the five tasks exist to satisfy exactly that.
4. **`--dry-run` producing no change**, with `git status` after it.
5. **The printed path list**, shown to be complete: every path the command changed appears in it.
   The natural control is to stage exactly that list and find `git status` clean.
6. **R7**: the step-7 section rewritten to call the command while still listing the five tasks.
7. Host tests for the refusal conditions, `fmt --check`, and `consistency-check --all` in the real
   tree (unchanged by this line, so it should be 21 PASS before and after).

## 6. Stop and escalate if

- Moving a row between the two tables cannot be done without rewriting prose.
- `rfcs/README.md`'s shape turns out to vary between milestones in a way that makes R3's "cannot be
  located" fire on a legitimate tree.
- The tool would need to parse an RFC's body for anything beyond its line-3 `Status:`.

## 7. Not yours

`rfcs/ERRATA.md` — **E-116** is CLOSED and this line does not reopen it; it removes the residue
that entry recorded as unfixed, and whether that warrants a note in the entry is the architect's
call. `v0-release-cycle.md` is the architect's **except** for R7's step-7 rewrite, which is yours
because it must describe what you built.
