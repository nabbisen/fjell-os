# RFC 000 — RFC Lifecycle Policy

## What this document is

This is a **merge**, adopted 2026-08-03 as RFC-0.25-002. The owner directed
replacement of this file with a general-purpose, portable rules document
written for any project starting an `rfcs/` directory from scratch. Re-
confirmation found that a verbatim replacement would delete five rules this
project actively depends on — one of them, the drift/errata rule, in use
today (RFC-0.24-001 ships `Implemented-with-Errata`, a state the source
document does not define).

So this document takes its **structure** — folder layout, the 5-folder
variant, transitions, handoff conventions, README integrity, cross-reference
discipline, anti-patterns — from that source, and keeps this project's own
five rules verbatim. Every place this document departs from the source is
called out inline, with the reason, rather than left for a reader to guess
whether the difference was deliberate.

**The five rules kept, by name**, so a reviewer can check this document
against that list rather than read it for general resemblance:

1. `Accepted`
2. `Implemented-with-Errata`
3. `Closed`
4. The drift/errata rule
5. The required-sections list (9 items)

This file has no machine-checked `Status:` field, by the same design as
before the merge — `tools/fjell-consistency-check/src/status.rs` documents
this file as one of two deliberate exceptions. Its status is: in effect,
and self-applying (see [§ Self-application](#self-application)).

## Lifecycle states

An RFC is in exactly one of the following states at any time:

| State | Meaning |
|---|---|
| **Proposed** | Filed; open for review and discussion. Implementer should not yet start — the design may change. |
| **Accepted** | Approved by the owner; implementer may begin. Design is settled but the work has not shipped. |
| **Implemented** | Code merged; smoke tests pass; the RFC's normative text is fully satisfied by what shipped. Historical record from here. |
| **Implemented-with-Errata** | Code merged, but the RFC's normative text claims more than what shipped. The divergence is recorded in `docs/rfcs/ERRATA.md` — see [§ The drift and errata rule](#the-drift-and-errata-rule) below. |
| **Superseded** | A later RFC replaces this one. The replacement's identifier is recorded in this RFC's Status field. |
| **Withdrawn** | The owner or architect decided not to pursue this RFC. The work will not happen. |
| **Closed** | Implemented (or Implemented-with-Errata, once its errata is tracked to closure) and documented; no further action needed. |

```
Proposed → Accepted → Implemented → Closed
                   ↘ Withdrawn
        Implemented → Implemented-with-Errata → Closed
                   ↘ Superseded
```

The source document's simpler four-state model (Draft, Proposed,
Implemented, Withdrawn, Superseded) does not have a way to say "this shipped,
but not quite as designed" — the RFC-0.24-001 audit's very first finding was
an instrument (`release-rehearsal` Gate 4) marked sound on evidence that
did not demonstrate what it claimed, and this project's answer to *that*
shape of problem is `Implemented-with-Errata`, not silence. `Closed` exists
because "shipped" and "the paperwork is finished" are observably different
events here — an `Implemented-with-Errata` RFC stays open until its
`ERRATA.md` entry is dispositioned, which can be much later.

This project has no `Draft` state in practice — RFCs are proposed directly
by the architect — so it is not carried forward. Nothing prevents adding it
later if that changes.

### The drift and errata rule

> **An RFC may not be marked Implemented if its normative text makes a claim
> the merged code does not satisfy.** In that case it is marked
> **Implemented-with-Errata**, and an entry is added to `docs/rfcs/ERRATA.md`
> naming: the RFC, the claim, what actually shipped, and the tracking RFC (if
> any) for closure. No RFC may silently carry drift into a release.

This is this project's most distinctive governance rule and the one this
merge exists to protect. `docs/rfcs/ERRATA.md` — seventeen entries at time
of writing (E-001 through E-017) — exists because of it. `release-rehearsal`
Gate 7 (errata register, zero `OPEN`) and Gate 12's `errata-limitations`
subcheck both rest on it. The source policy has no equivalent, because most
projects it targets have no errata register.

## Folder layout — the 5-folder variant, as adopted

```
rfcs/
  README.md       ← index; lists every RFC by state
  proposed/       ← Proposed RFCs — under review
  accepted/       ← Accepted RFCs — signed off, not yet shipped
  done/           ← Implemented / Implemented-with-Errata / Superseded /
                     Withdrawn / Closed RFCs — final disposition
  archive/        ← reserved for a future Withdrawn/Superseded split from
                     done/, should the project ever want one; currently
                     empty, and Withdrawn/Superseded RFCs live in done/
  handoffs/       ← optional; companion execution docs, status inherited
                     from the matching RFC
```

**The folder is the source of truth for an RFC's lifecycle state.** This is
the rule two files in this repository have cited as coming from this
document since before this document said it: `rfcs/README.md` and
`tools/fjell-consistency-check/src/rfc_status_folder.rs` both carried the
sentence "folder is the source of truth for state (see RFC 000)" while the
prior version of this file mentioned folders zero times. Both citations
were false — not because either document was wrong about the *rule*, but
because the rule had never actually been written down anywhere. This
sentence, right here, is that rule, stated for the first time. Both
citations are corrected to point at it (RFC-0.24-001's E-016 — no
instrument verifies a document's claims — landed inside this project's own
governance document, and this merge is also that finding's repair).

A file's `Status:` field must be kept consistent with its folder, but if the
two ever disagree, **the folder wins**. `rfc_status_folder.rs` enforces this
mechanically: it reads every `.md` file under `proposed/`, `accepted/`, and
`done/`, extracts each one's `Status:` keyword, and fails if the keyword is
not one this folder is allowed to hold.

**Why `archive/` exists but is empty.** The source document's four-folder
form has one folder, `archive/`, for both Withdrawn and Superseded RFCs. This
project's original policy filed both under `done/` alongside Implemented
ones instead — no RFC has ever been formally withdrawn or superseded here.
`rfcs/README.md` documented `archive/` as if it existed for some time before
this merge, which was itself a small instance of the same defect this whole
document exists to close (a claim the repository did not match). The folder
now exists so the documented layout and the tree agree; `Withdrawn` and
`Superseded` RFCs continue to be filed under `done/` per the state table
above, since `rfc_status_folder.rs`'s `DONE_STATUSES` already accepts both
and no RFC has ever needed to move. Splitting them into `archive/` is future
work, not required by this merge.

**Why `accepted/` fits here**, by the source document's own test for when
the 5-folder variant is worth the extra folder: "the maintainer signed off"
and "the implementer finished" must be genuinely distinct, dated events, not
the same person doing both. In this project's actual workflow, the architect
proposes, the owner accepts, a separate implementation model builds, and the
architect reviews — four distinguishable roles across three of those events.
`accepted/` earns its place.

`proposed/` accepts only `Proposed`; `accepted/` accepts only `Accepted`.
Before `accepted/` existed, `proposed/` had to tolerate `Accepted` too,
because there was nowhere else to put a signed-off RFC — that tolerance
would now be a hole (an `Accepted` RFC left behind in `proposed/` would pass
unnoticed), so it was removed in the same change that created the folder.

## Naming and numbering

This project uses two coexisting schemes, and departs from the source
document's "sequential from 001, never reused" rule deliberately:

- **`NNN-slug.md`**, flat, sequential from `000` — RFCs `000` through `061`,
  **62 files**, all historical. `000` was this document.
- **`RFC-<milestone>-NNN-slug.md`**, numbered per milestone, from v0.3
  onward — **101 files** as of this merge (2026-08-03; the RFC that
  proposed this document measured 99 at drafting time, two more landed
  before this document was written).

The break between the two schemes is historical and frozen: existing files
keep their names either way (the source document's own anti-pattern is
renaming during a reorganisation — renumbering or renaming any of the 163
files here would silently break every commit message, release record, and
`ERRATA.md` entry that already points at one of them). New RFCs use the
prefixed form.

**The prefix is a batch label, not a release claim.** `RFC-v0.7.4-001`
records that the RFC was *raised* under the v0.7.4 milestone plan — not that
it shipped in v0.7.4. Milestones get re-planned after RFCs are filed under
them; an immutable filename cannot track a mutable fact.

Measured 2026-08-03: **nine of the sixty-four** prefixed RFCs that record a
shipped release shipped under a different milestone than their prefix names —
**14%** — three of them *earlier* than their prefix (e.g. `RFC-v0.7.4-001`,
DMA Lifetime Safety, shipped in v0.7.1). These are not naming mistakes; they
are the scheme working exactly as designed, and it will keep producing them.

**The denominator is sixty-four, not the full prefixed population.** Of the
101 prefixed RFCs, 37 have not shipped and so have no Shipped column to
disagree with — they cannot diverge, and counting them would understate the
rate. RFC-0.25-002 originally stated "nine of ninety-nine"; that denominator
included RFCs outside the population and was corrected in review. Anyone
re-deriving this figure must use the same denominator: **prefixed RFCs that
record a shipped release.**

**`rfcs/README.md`'s "Shipped" column is the authoritative record of where
an RFC landed** — never the filename prefix. Nothing currently checks that
an RFC's prefix and its README "Shipped" entry are consistent, or that the
"Shipped" entry is itself accurate; that is a concrete instance of
RFC-0.24-001's E-016 (no instrument verifies a document's claims) and is
recorded there as a 0.25 candidate, not built here.

## Required sections

Each RFC must contain:

1. **Title** — one sentence
2. **RFC ID** — matches filename
3. **Status** — one of the states above
4. **Problem** — what is broken or missing; evidence (file + line if applicable)
5. **Proposed fix** — concrete change; code snippets where helpful
6. **Rationale** — why this fix and not alternatives
7. **Impact** — crates affected; backward compatibility
8. **Test plan** — how correctness is verified after the fix
9. **Implementation notes** — any constraints the implementer must know

The source document explicitly declines to require any particular RFC
shape ("makes no claim about what an RFC contains"), on the reasoning that
different projects legitimately want different templates. This project has
had a fixed template since its first RFC, and the required-sections list is
part of what an RFC *is* here — a proposal missing one of these is
incomplete, not merely differently organised. Kept verbatim from the
pre-merge policy.

## Companion handoffs

A handoff is an optional implementation companion to an RFC, useful when an
RFC is large enough that implementers need a separate execution package:
implementation notes, slice/PR sequencing, acceptance checks, required
demonstrations, escalation triggers.

A handoff answers a different question from the RFC:

- the RFC records **what decision was made and why**;
- the handoff records **how to implement and verify it safely**.

Handoffs must not override RFC decisions. If handoff work uncovers a design
conflict, the RFC is updated first, then the handoff to match — this
project's standing instruction to every implementer is "if you find a design
conflict, stop and escalate; do not resolve it in code," which is this rule
applied at the point of execution.

```text
rfcs/handoffs/<rfc-id>/
  implementation-handoff.md
  README.md                 ← optional
```

`handoffs/` is not split into `proposed/`, `accepted/`, `done/`, or
`archive/`. A handoff's status is inherited from the matching RFC's current
folder and `Status:` field. If the RFC moves, the handoff's meaning moves
with it without any file of its own moving. Do not manage handoff status
separately — the source document's anti-pattern of a handoff gaining its
own parallel lifecycle applies here unchanged.

## Cross-references between RFCs

When one RFC references another, use a relative path reflecting the
target's *current* folder:

```markdown
See [RFC-v0.22-001](../done/RFC-v0.22-001-gate-integrity.md) for the prior work.
```

Cross-references break when an RFC moves between folders. When an RFC moves,
run `grep -rl "<filename>" rfcs/` before the move and update every match in
the same commit. Done this way for this merge's own folder migration: three
files referenced `proposed/RFC-0.25-00*` before the move, zero after —
reported by count, not asserted as "links updated."

## Review and transitions

State transitions are operations performed by the architect (proposing,
reviewing) or the owner (accepting):

- **Open.** New file added to `proposed/`.
- **Accept.** Owner signs off; move `proposed/` → `accepted/`; update
  `Status:`.
- **Ship.** Implementation lands; move `accepted/` → `done/`; update
  `Status:` to `Implemented` (or `Implemented-with-Errata` — see
  [§ The drift and errata rule](#the-drift-and-errata-rule) — if the
  normative text overclaims what shipped).
- **Close.** An `Implemented-with-Errata` RFC's tracked errata is
  dispositioned to `CLOSED` or `ACCEPTED`; update `Status:` to `Closed`.
- **Withdraw.** The owner or architect decides not to pursue the RFC;
  update `Status:` to `Withdrawn` with a one-line reason. Files under
  `done/` today per [§ Folder layout](#folder-layout--the-5-folder-variant-as-adopted)'s
  note on `archive/`.
- **Supersede.** A later RFC replaces this one; update `Status:` to
  `Superseded by RFC <id>`, with a reciprocal note in the replacement.

### Granularity of transitions

A single RFC does not need to enumerate every sub-feature to qualify as
Implemented. Partial implementation is fine if the partial work captures the
RFC's main design decision; deferred work either gets a follow-up RFC or is
logged as an explicit deferred note — or, if the RFC's own text claims more
than what shipped, it is `Implemented-with-Errata`, not silently accepted as
`Implemented`. Do not keep an RFC in `proposed/` or `accepted/` indefinitely
because one open question remains.

## README integrity

`rfcs/README.md` is the index. It must:

1. List every RFC across all folders, grouped by state.
2. Use relative links reflecting each RFC's current folder.
3. Be updated in the same commit that moves an RFC between folders.
4. Indicate where a handoff exists, without treating the handoff as a
   separate lifecycle item.
5. State every folder that exists, and no folder that does not — a
   documented-but-absent folder is the same defect as a citation of a rule
   that is not written (see [§ Folder layout](#folder-layout--the-5-folder-variant-as-adopted)'s
   note on how `archive/` was documented before it existed).

## Optional CI invariants — what this project actually checks

The source document lists these as invariants worth checking *if* a project
has CI on its RFC directory. This project does, so the list here is not
aspirational — it names what `tools/fjell-consistency-check`'s
`rfc-status-folder` subcheck (`release-rehearsal` Gate 12) enforces today,
plus what remains a gap:

- **Enforced:** every file under `proposed/`, `accepted/`, `done/` that
  declares a bold `Status` field (colon- or period-labelled) carries a
  keyword legal for its folder. Demonstrated failing both directions before
  being
  trusted, per this project's standing rule that a strengthened gate must
  be observed failing before it is relied on (RFC-v0.22-001): an `Accepted`
  RFC left in `proposed/` fails; a `Proposed` RFC placed in `accepted/`
  fails.
- **Not enforced** (RFC-0.24-001 E-016, recorded as 0.25 candidates, not
  built here): broken relative links between RFCs; every RFC in
  `rfcs/README.md` existing at its linked path and vice versa; an RFC's
  filename prefix matching its README "Shipped" column; filename-to-slug
  consistency; handoff directories corresponding to a real RFC.

A handful of files carry no `Status:` field at all, by documented design —
this file, and the `v0.7.x-index.md` overview page, which is not itself an
RFC. `rfc_status_folder.rs` skips files with no Status field rather than
failing them.

## Anti-patterns

Patterns that look reasonable but cause long-term harm:

### Deleting completed RFCs to "clean up"

RFCs capture the *why* — alternatives considered, trade-offs weighed, open
questions resolved. Code captures the *what*. Both are needed. Never delete;
move to `done/` and leave it there.

### Renumbering or renaming RFCs during reorganisation

External references — commit messages, release records, `ERRATA.md`
entries, other RFCs' cross-references — all point at a specific filename.
Renaming or renumbering breaks every one of them silently. This project has
163 such files; the naming scheme documented in [§ Naming and
numbering](#naming-and-numbering) exists specifically because a uniform
rename was rejected as the wrong fix for the scheme's own inconsistency.

### Status fields that lie

If an RFC's `Status:` field disagrees with its folder, the folder wins, but
the mismatch still causes friction for anyone reading the file directly.
Update `Status:` in the same commit that moves the file.

### Letting cross-references rot

`grep -rl "<filename>" rfcs/` before every move, every match updated in the
same commit. Reported counts, not an unverified claim of "links updated."

### A citation asserting a rule its target does not contain

Found twice in this project by 2026-08-03: `rfcs/README.md` and
`rfc_status_folder.rs` both cited this document for
folder-as-source-of-truth before this document said it; `rfcs/README.md`
documented `archive/` before the folder existed. Both are instances of the
same shape RFC-0.24-001's audit spent a milestone finding elsewhere in the
project's *instruments* — here it was the project's *governance document*
that was stale, and everything citing it inherited the staleness silently.
The fix in both cases: write the rule down where it is claimed to live, or
stop citing it.

### Turning handoffs into a second RFC lifecycle

A handoff with its own `proposed/`/`done/` subfolders, or a handoff whose
stated status disagrees with its RFC's, forces a reader to guess which is
authoritative. Handoffs are companions; their state is always the matching
RFC's folder and `Status:` field.

### Letting handoffs override RFC decisions

If implementation reveals a design problem, the RFC is patched or superseded
first, then the handoff updated to describe execution of the *current* RFC.
This project's standing instruction to every implementer — stop and
escalate a design conflict rather than resolve it in code — is this
anti-pattern's fix, applied at the point where it would otherwise happen.

### Formalising `accepted/` where the roles collapse

Not this project's situation (see [§ Folder layout](#folder-layout--the-5-folder-variant-as-adopted)),
but worth naming for whoever next considers removing it: if "the owner
accepted" and "the implementer finished" ever collapse back into one event
here, `accepted/` becomes dead weight, and the 4-folder variant is the
better fit at that point. Revisit if that changes; don't remove pre-
emptively while the roles are genuinely separate.

## Self-application

This document describes its own placement: it lives in `rfcs/done/` as
`000-rfc-lifecycle-policy.md`, unchanged in filename and folder by this
merge, because the policy has been in effect for this project's RFC
directory since before this project needed to write it down this precisely.

The transition that produced *this* text was RFC-0.25-002: the owner
directed a verbatim replacement; re-confirmation found that would delete
five load-bearing rules; the RFC merged instead, keeping the five and
adopting the rest. The folder migration (`accepted/` and `archive/`
created; `RFC-0.25-001` and `RFC-0.25-002` itself moved into `accepted/`)
landed ahead of this document, by explicit owner direction, with the
`rfc_status_folder.rs` change landing in the same commit so the new folder
was never uninstrumented — the alternative would have been an `Accepted`
RFC sitting somewhere Gate 12 does not read while still reporting `PASS`,
exactly the class of defect the 0.24 line spent a milestone removing.

This document is the last piece: it makes the layout's citations true
rather than aspirational.

## Open questions

None at time of this merge. RFC-0.24-001's E-016 names three further
instances of "nothing verifies a document's claim" that this merge does not
build instruments for — thirteen broken relative links elsewhere in tracked
documentation, the filename-prefix-vs-Shipped-column consistency this
document now states normatively but does not check, and the audit's own
totals-table arithmetic. All three remain 0.25 candidates; adding an
instrument was this line's explicit non-goal. Future refinements to the
lifecycle itself — review SLAs, a `draft/` folder if shared drafting ever
becomes common, automated state-machine checks beyond `rfc-status-folder` —
land as follow-up RFCs referencing this one, per the process this document
itself defines.
