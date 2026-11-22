# RFC-0.47-002: The lifecycle policy's kept rules are written down, and a check can find them missing

**Status:** Accepted — accepted 2026-11-23
**Milestone:** 0.47
**Tracks.** **E-128** — the five kept rules landed (RFC-0.25-002, `be8cf6f`) and an untitled rewrite of
RFC 000 deleted them two months later (`4d8044a`). **This RFC's first draft diagnosed it as D1 never having
landed; that was wrong, the architect corrected it at the stage-1 review, and §"What is true now" is
rewritten to the verified history.** E-128's disposition is the architect's.
**Touches** *(indicative)*: `rfcs/done/000-rfc-lifecycle-policy.md`, `rfcs/ERRATA.md` (header only),
`tools/fjell-consistency-check/src/` (one new subcheck; `status.rs` and `rfc_status_folder.rs` read the
vocabulary from the policy instead of a constant), `rfcs/README.md`. **No kernel, ABI or prebuilt change.**
**Relates to:** **RFC-0.25-002** (D1 — the decision this *re*-lands; its own status is correct and unchanged), **RFC 000**, **E-114** (the register's
status lives in one place), **E-014** (an instrument reporting coverage it does not have).
**Not in this line:** judging whether any RFC's normative text *is* true (a reviewer's job; §"What a check
cannot do"), and **editing any RFC's status line** — those are the architect's (§D5).

*Every figure below was re-measured at `8cc6bcc`; the history in §"What is true now" was re-verified at the
stage-1 review's tip against `git show`.*

---

## What is true now

RFC-0.25-002 is `Implemented (0.25.0) — R1–R5 complete`. Its decision **D1** (`:102`–`:103`): *"Keep from the
in-repo policy: `Accepted`, `Implemented-with-Errata`, `Closed`, the drift/errata rule, and the
required-sections list."* Its acceptance checklist (`:257`–`:261`) requires *"All five kept rules present and
checked **by name**."*

**D1 landed, in full, and was verified by name.** `be8cf6f` (2026-08-15) made RFC 000 430 lines and its
message states *"All five kept rules verified independently by name rather than by reading for sense:
Accepted, Implemented-with-Errata, Closed, the drift/errata rule verbatim as a normative blockquote, and all
nine required sections."* **Then `4d8044a` (2026-10-21), whose entire message is *"RFC 000 life cycle policy:
revised"*, rewrote the file (537 insertions, 421 deletions) and removed all five** — the straight
replacement RFC-0.25-002 had analysed and rejected with a table of consequences. Measured with `git show`
across it (`4d8044a^` → `4d8044a`):

| In `rfcs/done/000-rfc-lifecycle-policy.md` | before | after |
|---|---|---|
| lines | 501 | 617 |
| the drift rule (`may not be marked`) | 1 | **0** |
| `Implemented-with-Errata` | 13 | **0** |
| `Closed` | 7 | **0** |
| `Test plan` (the required-sections list) | 1 | **0** |

**This is a regression, not a decision that failed to land, and my first draft got it wrong.** I took the
oldest hit of `git log -S` for the rule's text as the point at which it was written down, and never opened
the file at the commits between. Nothing noticed for two months and no erratum was filed.

Today none of the five is in the file:

| Kept rule | In RFC 000 now? | Measured |
|---|---|---|
| `Accepted` | **no** — appears once, as what *"some projects add"* | `rfcs/accepted/` exists; the gate requires `Accepted` in it; **every milestone since 0.25 uses it** |
| `Implemented-with-Errata` | **no** | **12 RFCs carry it**; all 12 are named in the register (1–12 times each) |
| `Closed` | **no** | **no RFC carries it**; `status.rs` and `rfc_status_folder.rs` accept it from a Rust constant |
| **the drift/errata rule** | **no** | its text is in two tracked files, both records of the decision to write it (`RFC-0.25-002:80`, its handoff `:28`–`:29`) |
| required-sections list | **no** | see §D3 — **restoring it as written would fail essentially every RFC** |

`git ls-files -z | xargs -0 /usr/bin/grep -ailn 'may not be marked [I]mplemented'` prints those two files, re-run
at this tip (the bracket keeps the command from matching the document that quotes it). **The text to restore
from is `4d8044a^:rfcs/done/000-rfc-lifecycle-policy.md`** (501 lines — the merged, project-specific version,
the drift rule at its `:71`–`:75`), **not** the 64-line pre-merge policy my first draft named: the merged
file already carries the five rules in this project's own wording, the 5-folder variant and the
cross-reference discipline. I use its text below where it was right and say where I did not.

### Three findings the handoff did not have

**F1 — the instruments enforce a vocabulary the policy does not define, in the wrong direction.**
`status.rs`'s `STATUS_KEYWORDS` (seven words) and `rfc_status_folder.rs`'s `DONE_STATUSES` are Rust
constants. `status.rs`'s own doc says *"RFC 000 … names the fixed set of lifecycle states"*; RFC 000 names a
*different* set — `Draft`, `Proposed`, `Implemented`, `Withdrawn`, `Superseded` — of which `Draft` is **not** in
the constant. **Answering the handoff's question:** `rfc-status-folder` *does* validate the status word
against a defined set — but the set is the constant, so the words are legal because code says so. That is the
inversion; this line's gate reverses it.

**F2 — the drift rule has a second live instance, found by deriving it.** Parsing the register, **17 of 127**
entry headings name an RFC as their subject in a parseable form. Of those, **two are live (`OPEN` or
`ACCEPTED`) against an RFC still marked plain `Implemented`: E-128 and E-004.** E-128's heading names
RFC-0.25-002 only because the first diagnosis did; **it is not an over-claim by that RFC** (§"What is true
now") and the architect rewrote the entry. **E-004** stands — `RFC-v0.12-002` *"real-board target
selection"*, whose register entry records *"no hardware was booted"* against an RFC that claims a validated
target, and which was `Implemented (v0.12.0)`. By the rule it should have been `Implemented-with-Errata`;
the architect has re-marked it. It is evidence that the rule was not being applied *before* it went
missing, not only after.

**F3 — only 17 of 127 headings name their RFC, so the rule's reverse half cannot be checked from the register
as it stands.** *"Every `Implemented-with-Errata` RFC is named in the register"* **is** derivable and is true
today (all 12). *"No RFC with a live erratum is plain `Implemented`"* needs the register to say which RFC an
entry is about, and 110 of 127 do not in a form a program can read. §D4 makes it forward-only on purpose.

---

## Requirements

**R1 — the state table is the vocabulary, and it is complete.** `rfcs/done/000-rfc-lifecycle-policy.md`'s
state table defines exactly the states in §D1, each with: what it means, what moves an RFC into it, and (for
`Implemented-with-Errata`) what it obliges. The paragraph at `:93` that calls `Accepted` formalisation
overhead *"for most projects"* is rewritten — this project has `accepted/` and uses it.

**R2 — the drift/errata rule is stated normatively, once, where §D2 says.** Text, restored from
`4d8044a^:rfcs/done/000-rfc-lifecycle-policy.md` (`:71`–`:75`) and tightened (its *"No RFC may silently carry
drift into a release"* is kept):

> **An RFC may not be marked `Implemented` if its normative text makes a claim the merged code does not
> satisfy.** It is marked `Implemented-with-Errata` instead, and an entry is added to `rfcs/ERRATA.md`
> naming the RFC, the claim, what shipped, and the tracking RFC. An RFC found to over-claim *after* it was
> marked `Implemented` is **re-marked** `Implemented-with-Errata` in the same change that files the entry.
> No RFC may silently carry drift into a release.

The last sentence of the middle clause is **new** and is the rule's missing half: the original governed the
moment of marking and said nothing about discovery, which is how E-004 came to carry `Implemented` over a
filed over-claim. (The regression case — the rule being *deleted* — is covered by R5.2, not by this sentence.)

**R3 — the required-sections list is re-decided from the evidence (§D3), not restored.**

**R4 — `ERRATA.md`'s header states the procedure and cross-references the rule**, so a reader who arrives at
the register is told what files an entry and what status it implies for the RFC.

**R5 — the check: `rfc-policy`, the 26th `consistency-check` subcheck** *(27th if 0.47-001's lands first)*.
It derives, it does not list:

1. **Vocabulary parity, three-way.** The set of states **defined in RFC 000's table** is parsed from the
   table. It must equal `STATUS_KEYWORDS` and be a superset of `DONE_STATUSES`; **`STATUS_KEYWORDS` and
   `DONE_STATUSES` are changed to read the policy's table** (one source) rather than be compared with it, so
   the inversion in F1 ends rather than being monitored. Every RFC's extracted status keyword must be in the
   set. A word in code the policy does not define, or a defined state the code does not know, **fails**.
2. **The rule is present, by name, at both ends.** RFC 000 contains the normative sentence (the check holds the
   sentence's first clause as a constant — *this one text is the rule, and an edit that removes it must be
   deliberate*) in the definition of `Implemented`; `rfcs/ERRATA.md`'s header contains the cross-reference.
   This is the half RFC-0.25-002's own checklist asked for and never got: **"present and checked by name."**
3. **`Implemented-with-Errata` ⇒ named in the register.** Every RFC whose status keyword is
   `Implemented-with-Errata` has its id in `ERRATA.md`. **Green today for all 12.**
4. **Forward only: a live erratum's subject RFC is not plain `Implemented`** — for entries carrying the new
   `- **RFC:**` field (§D4). Not retro-applied to the 110 entries without it.

---

## The decisions this RFC asks for

**D1 — the vocabulary. Proposed set, with reasons; not D1 restored by default.**

| State | Verdict | Reason |
|---|---|---|
| `Proposed` | **keep** | defined; every RFC enters here |
| `Accepted` | **keep, define** | used at every cut; the folder and the gate depend on it. The one place RFC 000 is wrong is the sentence that says projects like this one do not need it |
| `Implemented` | **keep, amend** | its definition gains the drift condition (R2): shipped **and** the normative text is satisfied |
| `Implemented-with-Errata` | **keep, define** | 12 RFCs carry it, the register depends on it, the gate already accepts it. **Permanent once marked** — see below |
| `Superseded` | keep | defined, legal, **zero users**; costs nothing and is a real end state |
| `Withdrawn` | keep | same |
| `Draft` | **drop from the policy** | defined by RFC 000, **absent from the gate's constant, used by no RFC**; an RFC enters the repository as `Proposed`, so `Draft` is not a repository state — saying so removes a defined state nothing can be in |
| `Closed` | **drop** | **no RFC has ever carried it**, and it duplicates what the register already says. E-114 fixed *one place where status lives*; `Closed` would be a second closure vocabulary for RFCs beside the register's `CLOSED`. The old policy used it as *"Implemented + documented; no further action"* — an assertion nothing could ever check |

*On `Implemented-with-Errata` being permanent.* It records a fact about the RFC **at the time it shipped**
(*its text claimed more than the tree delivered*), not the register's current disposition. When every erratum
naming it is `CLOSED`, the RFC's *text* may be corrected; the **status stays**, because re-marking would erase
the evidence the rule exists to keep. The alternative — returning it to `Implemented` on closure — makes the
status a moving summary of the register, which is exactly the duplicate source E-114 removed. **Your ruling:**
if you prefer the old `→ Closed` transition, that is the one place I would be restoring D1 rather than
re-deciding it, and I would want the transition's trigger stated as checkably as R5.3 states the rule.

**D2 — where the rule lives.** The handoff recommends `ERRATA.md`'s header with a cross-reference from
RFC 000. **I would argue the other way, and ask you to rule.** The rule's *trigger* is the choice of a status
word, and its *content* is a condition on the meaning of `Implemented`; a definition belongs where the term is
defined, or the table says "the work has shipped" while a rule elsewhere says it also must not over-claim —
two statements of one state in two documents, which is the failure this RFC repairs. **Proposal:** the
normative sentence in RFC 000's definition of `Implemented` and `Implemented-with-Errata`; **`ERRATA.md`'s
header states the procedure** (what files an entry, and that the RFC is re-marked in the same change) **and
links to it**, so the reader who meets it there is told and pointed. R5.2 checks both ends, so the choice costs
nothing in enforceability — only in where a reader first looks. If you keep your recommendation, R2 and R4
swap places and R5.2's two checks swap files; nothing else changes.

**D3 — the required-sections list: re-decided, because restoring it would fail the tree.** The old list
(nine sections: title, id, status, *Problem*, *Proposed fix*, *Rationale*, *Impact*, *Test plan*,
*Implementation notes*) **as literal headings is carried by 5 and 3 of 173 RFCs**. By cohort, measured:

| Field / section | RFCs ≥ 0.40 (23) | RFCs < 0.40 (150) |
|---|---|---|
| `**Status:**` | 23 | 106 |
| `**Milestone:**` | 23 | 74 |
| `**Tracks`  | 23 | 57 |
| `**Touches` | 23 | 58 |
| a `Requirements` section | 23 | 40 |
| `**Evidence:**` | 6 | 0 |

**Proposal:** required for RFCs **numbered 0.47-001 and after**, **not retro-applied**: the **`Status`**,
**`Milestone`** and **`Tracks`** header lines and a **`Requirements`** section — the four every recent RFC
already carries. `**Evidence:**` stays governed by the existing `rfc-evidence` rule for `Implemented` RFCs.
*Why not the old nine:* the project's RFCs have a different, consistent, evolved shape; a list that 170 of 173
documents violate is not a rule, it is a permanent exception. *Why forward-only:* a rule that fails 150
historical documents is a rule nobody can satisfy, and rewriting released records is what this project does not
do. **This RFC does not add a gate for D3** — the data justifies the list, but the check is a second, larger
line and belongs to a later milestone if you want it; R3 states the list and says it is *advisory until a
check exists*, which is true. (If you want it gated now it is one more judgment in R5; say so.)

**D4 — the register gets one structured field, forward-only (R5.4).** Entries gain `- **RFC:** <id>` (the RFC
whose normative text over-claims) or `- **RFC:** none` (drift that is not an RFC's text — E-125, E-126). It is
required for entries filed from E-128 on, and **backfilled by the architect on the live entries (E-004 and the
other `ACCEPTED`/`OPEN` ones — eight in all)**; the other ~110 closed entries are not touched. *Why a field and
not a heading parse:* F3 — 17 of 127 headings name their RFC, so a heading parse would give a gate that
checks 13% of the register and calls itself a check. *The alternative is to leave R5.4 out and say that the
rule's reverse half is review-enforced only* — honest, smaller, and what the register is today.

**D5 — status lines are the architect's. RULED (split):** RFC-0.25-002 **stays `Implemented`** — its text was
satisfied by the merged code when it shipped, and `Implemented-with-Errata` records an RFC that claimed more
than the tree delivered; applying it would record a false fact about that RFC and misuse the rule this line
restores. `RFC-v0.12-002` (E-004) **is re-marked `Implemented-with-Errata`**, done by the architect at the
stage-1 review. **My first draft asked for both; the first was wrong, for the reason this RFC's own D1 gives
for permanence** — the status is a fact about the RFC when it shipped. E-128 stays `OPEN`, tracking this RFC;
its disposition is the architect's.

---

## What a check cannot do, stated so a green is not over-read

* **It cannot judge whether an RFC's normative text is satisfied by the code.** That is the question the rule
  asks and it is a reviewer's. What R5 can do is make the rule's *consequences* checkable: the vocabulary is one
  source, the rule is present, a marked RFC is in the register, and (forward) a live over-claim is not
  hidden behind `Implemented`. **A green `rfc-policy` means the rule is written down and its bookkeeping
  holds; it does not mean no RFC over-claims.** E-004 shows both halves: the rule was not applied, and no
  check could have said so.
* **R5.2 checks that the rule's sentence is present, not that anyone follows it.** It is the check that would
  have caught `4d8044a` deleting the rules the day it landed; it is not a substitute for the rule.

## Negative cases, and what I will show red

1. **Vocabulary:** add a status word to `STATUS_KEYWORDS` that RFC 000's table does not define → red, naming
   it; delete `Accepted` from RFC 000's table → red (a defined-and-used state undefined); an RFC with a status
   word outside the set → red, naming the file.
2. **The rule's presence:** remove the normative sentence from RFC 000 → red; remove the cross-reference from
   `ERRATA.md`'s header → red.
3. **`Implemented-with-Errata` ⇒ named:** plant an RFC with that status that no entry names → red, naming it.
4. **Forward field:** an `OPEN` entry with an `**RFC:**` field naming an RFC that is plain `Implemented` →
   red, naming the entry and the RFC; the same entry naming an `Implemented-with-Errata` RFC → green;
   `**RFC:** none` → green. (Planted in a fixture; the live entries carry the field after the architect's
   backfill, D4.)
5. **The control:** the unmodified tree → green; and a run with **R5.2's sentence reworded
   but still present** → green, so the check is shown to hold the *presence*, not an exact byte string past its
   first clause.
6. A compile error is INVALID, per the standing rule; each red above must be a **named failure**.

## What this RFC does not do, and why

* **It does not edit any RFC's status** (D5) — the architect's.
* **It does not gate the required-sections list** (D3) — a later line, if wanted.
* **It does not backfill 110 closed entries** (D4) — released records are not rewritten.
* **It does not revisit the five-folder layout**, naming, or handoff conventions — those landed in
  RFC-0.25-002 and are not in question.
* **It does not claim the register is correct.** It claims the rule that the register's every disposition
  applies is, after this line, citable.

## Rulings (architect, stage-1 review, 2026-11-22)

D1 **accepted as proposed** (drop `Draft` and `Closed`; keep `Superseded`/`Withdrawn`; `Implemented-with-Errata`
permanent once marked; the old `→ Closed` transition is not restored). D2 **the architect withdrew their own
recommendation**: the normative sentence goes in RFC 000's definition of `Implemented`, `ERRATA.md`'s header
states the procedure and links — R2 and R4 stand as written. D3 **accepted, advisory, not gated in this line.**
D4 **accepted**; the architect backfills the live entries after the field's shape lands. D5 **split**, above.
**R5.2 is "the line's most important requirement" — keep it exactly as specified.** *Not asked for:* a gate on
commit messages, or a rule that RFC 000 may only be edited by an RFC.

## Reproduce

```
git ls-files -z | xargs -0 /usr/bin/grep -ailn 'may not be marked [I]mplemented'          # the two files
git show 4d8044a^:rfcs/done/000-rfc-lifecycle-policy.md                                # the text to restore from
git show 4d8044a --stat                                                                 # the commit that deleted it
git ls-files rfcs/done | xargs /usr/bin/grep -l -E 'Status[:.]\*\*.*Implemented-with-Errata'   # the 12
```

The cohort table and the 17-of-127 figure are computed by scripts over `git ls-files rfcs/done` and
`rfcs/ERRATA.md`'s headings and `## Summary` rows; the implementation commits them as the check's own tests
rather than as one-off output.
