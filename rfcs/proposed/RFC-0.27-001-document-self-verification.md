# RFC-0.27-001: Nothing verifies what our documents say about themselves

**Status:** Proposed — awaiting owner acceptance
**Milestone:** 0.27
**Tracks.** Cross-document agreement: the errata backlog, version claims, links,
and counts.
**Touches.** `tools/fjell-consistency-check`, `docs/rfcs/ERRATA.md`,
`ROADMAP.md`, `docs/verification/instrument-audit-closeout.md`, and whichever
documents the new checks find wrong.
**Relates to:** closes **E-016** and **E-023**; re-dispositions **E-014**,
**E-015**, **E-017**; RFC-v0.22-001 (Gate 12, which this extends),
RFC-0.24-001 (whose close-out named this a candidate).

## Summary

**This project has no scheduling source.** Asked what one was, the honest answer
was: the owner's decisions in conversation, plus whatever the previous line's
review happened to find. Neither is written down before the fact.

The one field that could serve — `ERRATA.md`'s tracking column — **is itself
stale**: E-014, E-015, E-016 and E-017 all still read *"0.25 candidate"*. 0.25
shipped on 2026-08-16 and 0.26 on 2026-08-27. Four of thirteen errata point at a
milestone that has passed, carried forward silently through two releases,
because nothing reads that column.

That is the same defect as the README sitting five releases stale at `0.21.3`
with five wrong counts, and the same defect as ROADMAP advertising four shipped
milestones as *"in progress"* and *"planned"*. **Three instances in one session,
all found by the owner reading, none by any check.**

This RFC makes the backlog derivable and enforced, and closes the family.

## Motivation

### The scheduling problem, concretely

"What is in 0.27?" is currently answerable only by reconciling three documents
by hand — `ERRATA.md`'s tracking column, the audit close-out's §6 candidate
list, and `ROADMAP.md`'s 0.27 section. **The architect added the third one on
2026-08-27 without noticing the duplication he was creating.**

None is authoritative, none cites the others, and one is stale.

### Why the errata register is the right source

It already is one, in practice. Every finding since E-011 landed there, each
carries a disposition, and **Gate 7 enforces `0 OPEN`** — which is why the
0.26 line could not cut a release while the ABDD path was dead. The register is
the only planning artefact in this project that already has teeth.

What it lacks is a tracking field that can be *read*. Today it holds prose:

```
0.25 candidate (recorded, not fixed)
RFC after v0.23.0 (recorded, not fixed)
0.27 candidate, with E-016 (recorded, not fixed)
v0.21.3-001 (v0.22 disposition)
```

Four different shapes, none parseable, one of them naming a milestone that
shipped two releases ago.

### E-023 already specified the missing check

RFC-v0.7.1-001, marked `Implemented (v0.7.1)`, specifies a release tool that
*"greps for stale version mentions outside `CHANGELOG.md`"* and *"exits non-zero
on any inconsistency."* Neither was built. **That check is exactly what would
have caught the README.**

This RFC does not invent a requirement. It builds one that has been specified,
marked implemented, and absent for nineteen releases.

## Scope — five subchecks, one family

Added to `fjell-consistency-check`, joining the four Gate 12 already runs.
Each is independently reviewable and independently revertible.

| # | Subcheck | Closes |
|---|---|---|
| **S1** | `errata-tracking` — the backlog becomes derivable | the scheduling gap |
| **S2** | Normalise the tracking column; re-disposition E-014/E-015/E-017 | the stale data S1 catches |
| **S3** | `version-currency` — no stale version claims outside `CHANGELOG.md` | **E-023** |
| **S4** | `doc-links` — every relative link in a tracked `.md` resolves | **E-016** (13 broken links) |
| **S5** | `doc-counts` — counts a document asserts about the tree are true | **E-016** (index counts) |

### S1 — `errata-tracking`

Three properties:

1. **Every tracking value parses** as either an RFC identifier
   (`RFC-0.26-004`) or a bare milestone (`0.27`). Prose is rejected.
2. **No erratum names a milestone that has already shipped.** This is the one
   that matters — it fires on E-014/015/016/017 today.
3. **RFC ↔ erratum agreement is bidirectional**: an RFC claiming to close
   `E-0NN` is named by `E-0NN`'s tracking field, and vice versa. Same shape as
   `errata-limitations` and `handoff-status`.

**Design question, not pre-decided:** how does the check know which milestones
have shipped? Candidates are `git tag`, `docs/release/records/*.md`, or
`CHANGELOG.md` headings. The release records are the most self-contained and do
not make an instrument depend on git state — but **state the choice and why**;
do not just pick one.

### S2 — the data, *after* S1

Normalise every tracking value to the parseable form, moving the parenthetical
commentary into the entry body where it belongs.

**Re-disposition E-014, E-015 and E-017 honestly.** They have been "0.25
candidate" through two releases. Each gets a real milestone or an explicit
statement that it is unscheduled — *"unscheduled"* is a legitimate value and far
better than a stale one.

### S3 — `version-currency`

No tracked document asserts a version other than the current workspace version,
outside `CHANGELOG.md` and `docs/release/records/` (which are historical by
design).

This is E-023's specified-but-never-built check.

### S4 — `doc-links`

Every relative link in a tracked `.md` resolves to an existing path. The audit
recorded **13 broken links**; expect that number to have moved.

### S5 — `doc-counts`

Where a document asserts a count about the tree, it is true. Start with
`rfcs/README.md`'s file counts — which drifted from 162 to 166 and were
corrected by hand on 2026-08-27.

**Scope this deliberately.** A general "verify every number in every document"
check is not buildable. Pick the counts that have actually drifted and are
mechanically derivable; **say which you excluded and why.**

## Design decisions

### D1 — S1 before S2, and do not fix the data first

**E-014, E-015, E-016 and E-017 are broken input sitting in the tree right now.**
That is a free demonstration on real committed data, and it will be destroyed
the moment the column is normalised.

Build S1, run it, **capture it failing on those four errata**, and only then fix
them. Exactly the sequence RFC-0.24-002 Slice 3 required for the
`asm-instruction` tag, and for the same reason.

### D2 — Subchecks of the existing tool, not a new one

Gate 12 already runs four cross-document checks. These are five more of the same
kind. A new binary would need its own gate wiring and would split a family that
belongs together.

### D3 — Properties, not implementations

S3, S4 and S5 state what must be true, not how. The mechanism is the
implementer's, per the pattern that worked in RFC-0.24-002 Slice 5 and
RFC-0.24-003 R5.

### D4 — Documents cite the backlog; they do not restate it

Once S1 makes the backlog derivable, `ROADMAP.md` and the audit close-out's §6
**cite** it. The 0.27 section becomes a pointer, not a list.

This is the part that stops the triplication recurring, and it is easy to skip
because it is documentation rather than code.

## Non-goals

- **Fixing E-014, E-015 or E-017's underlying defects.** This RFC
  re-dispositions them; it does not build the literal-predicate answer or
  reconcile CI's crate list.
- **E-022** (the kernel IPC contract). Separate line.
- **E-019 / RFC-0.26-003.** Still open, unaffected.
- A general document-linter. Five named checks, each with a defect on the record.
- Gate 12 `syscall-surface` must stay **35/29/6**.

## Risks

| ID | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R1 | The data is fixed before S1 can be demonstrated failing on it | **High** | **High** | D1. The four stale errata are the demonstration; normalising first destroys it |
| R2 | S4/S5 turn up more broken links and counts than expected, and the line balloons | **High** | Medium | Fix what is mechanical; **record what is not** rather than chasing it. A remaining broken link recorded is fine; an unrecorded one is not |
| R3 | A check is written to pass on today's tree rather than to be correct | Medium | **High** | Each subcheck demonstrated failing on deliberately broken input, per RFC-v0.22-001 |
| R4 | S5 is scoped as "verify every number" and becomes unbuildable | Medium | High | D3 and the explicit instruction to say what was excluded |
| R5 | D4 is skipped because it is only documentation | Medium | Medium | It is the requirement that prevents recurrence; it is listed in the acceptance criteria for that reason |

## Acceptance criteria

- [ ] **S1 demonstrated failing on E-014/015/016/017 as they stand today**,
      captured *before* S2 normalises them.
- [ ] Each of S1–S5 demonstrated failing on deliberately broken input.
- [ ] Every tracking value parses; **no erratum names a shipped milestone**;
      RFC ↔ erratum agreement holds both directions.
- [ ] E-014, E-015 and E-017 re-dispositioned to a real milestone or explicitly
      `unscheduled`.
- [ ] **E-016 and E-023 → `CLOSED`**, `ERRATA.md` and `v1-limitations.md` edited
      in the same commit.
- [ ] `ROADMAP.md` and the audit close-out **cite** the backlog rather than
      restating it; the 0.27 candidate list exists in exactly one place.
- [ ] Gate 12 reports the new subchecks; `syscall-surface` still **35/29/6**.
- [ ] `cargo xtask release-rehearsal` green; `cargo xtask test-all` 21/21.
- [ ] `cargo fmt --all --check` clean — run, not predicted.

## What this is really about

Every document in this project that makes a claim about itself has been wrong at
some point, and every one of those was found by a person reading rather than by
a check: the README's five counts, ROADMAP's four shipped-but-"planned"
sections, the audit's own totals table summing to 54 while stating 56, the RFC
index's file count, and now the errata backlog pointing at a shipped milestone.

The 0.24 audit asked *what would make this instrument report success without
having checked?* — of instruments. **These documents were never instruments, so
nobody asked.** They make claims, the claims are relied on, and nothing tests
them.

This line asks the question of the documents.
