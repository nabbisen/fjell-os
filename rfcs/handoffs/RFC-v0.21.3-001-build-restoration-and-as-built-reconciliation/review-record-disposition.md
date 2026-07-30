# Review Record — RFC-v0.21.3-001 disposition, and portfolio review

**Reviewer:** architect
**Reviewing:** `review-request-rfc-disposition.md`
**Commit reviewed:** `0a2bcfc`
**Date:** 2026-07-30

## Outcome

**Approved. RFC-v0.21.3-001 is dispositioned: Implemented (v0.21.3).**

One stale status field was found and fixed by me (§3) — it was in my document,
not theirs.

This is an RFC disposition point, so it also triggers the portfolio and roadmap
review required by the governance policy. That is §5, and it contains a
mandatory report to the owner.

## 1. Verified independently

| Claim | Method | Result |
|---|---|---|
| RFC moved, status updated | `ls rfcs/proposed/`; read Status field | **Confirmed** — in `done/`, `Status: Implemented (v0.21.3)` |
| Reference sweep complete | `grep -rn "rfcs/proposed/RFC-v0.21.3-001" --include="*.md"` | **Confirmed** — zero live references remain |
| Index re-filed | read `rfcs/README.md` | **Confirmed** — under Implemented, Shipped `v0.21.3`; Proposed now lists only RFC-v0.21.3-002 |
| Counts accurate | `ls rfcs/done \| wc -l` vs header | **Confirmed** — 155 claimed, 155 actual |
| Link integrity | mechanical check of all 158 relative links | **Confirmed** — 0 broken |
| `done/` ↔ index bijection | set comparison both directions | **Confirmed** — nothing in `done/` unlinked; nothing linked that is absent |

That last row is worth stating plainly: **every RFC on disk is indexed, and every
indexed RFC exists.** When this release line opened, the index listed 25 RFCs as
Proposed at paths that did not exist and undercounted `done/` by 55. The index
is now a true description of the folder, which is what RFC 000 requires of it.

## 2. Judgement calls — both correct

**Leaving the 5 dated `review-request-*` / `review-record-*` files' `proposed/`
references untouched.** Correct, and the reasoning given is the right one: those
are point-in-time submissions, and the RFC genuinely *was* in `proposed/` when
each was written. Rewriting them would misrepresent what was true at submission
time.

This distinction should be a standing rule, because it will recur:

> **Sweep live documents; never rewrite dated submissions.** A live document
> (index, README, CHANGELOG, design doc, handoff) is supposed to describe
> current state, so a stale path in it is a defect. A dated submission or review
> record describes a moment, so a path that was correct then stays as written.

**Folding the `rfcs/README.md` "Proposed — 1 RFC" header/count fix into the same
commit.** Correct. Removing a row from that table is what made the header wrong;
leaving it would have been creating drift while fixing drift. Fixing it there was
the smaller and more honest change. Correctly flagged rather than slipped in.

**Superseded notes added in place rather than rewritten.** Consistent with the
version-stamp ruling. The original v0.21.2-era claim stays legible above the
correction, so a reader sees both what was believed and what was true.

## 3. Correction — made by me, not required of the implementer

`implementation-handoff.md` line 5 still read:

```
**Status:** inherited from the governing RFC (Proposed — accepted for implementation, 2026-07-30)
```

The governing RFC is now Implemented (v0.21.3). RFC 000 states a handoff's status
is inherited from its RFC, and names "Status fields that lie" as an anti-pattern.
The sweep updated the *link* on line 3 of that same file and left the *status* two
lines below — an easy miss, and the handoff is an architect artifact, so it is
mine to keep accurate. Fixed to `Implemented (v0.21.3)`.

Noting it rather than fixing silently, because it is the fourth instance in this
release line of the same shape: a mechanical check passing while a documented
rule goes unmet. The others were the repro tier that could not fail, Gate 7 green
over an unrecorded ACCEPTED erratum, and the ABI gate green over stale scanner
paths. That pattern is now well-evidenced enough to be worth acting on, and it is
carried to v0.22 as a candidate theme in §5.

## 4. Answers to the three questions

1. **Fully dispositioned?** Yes, with §3 applied. No closing note is required —
   RFC 000 makes the folder plus the Status field the record, and both are now
   correct. Nothing further is owed on RFC-v0.21.3-001.
2. **Was the header/count fix right to fold in?** Yes. See §2.
3. **Wait for an RFC-v0.21.3-002 handoff before touching the release cut?**
   **Yes — wait, and do no prep work.** The release cut depends on two owner
   decisions that are still open (§5). Starting prep would mean guessing at them,
   and one of them changes an acceptance criterion. There is no useful work to
   pull forward here; idle is the correct state.

## 5. RFC disposition checkpoint — portfolio and roadmap review

Required at every disposition point.

### Portfolio status

| RFC | State | Notes |
|---|---|---|
| RFC-v0.21.3-001 | **Implemented (v0.21.3)** | This disposition |
| RFC-v0.21.3-002 | **Proposed** | v0 release cycle; 2 owner decisions open; **no handoff written yet** |

`rfcs/proposed/` now contains exactly one RFC. There is no other active
development theme.

### Blocked on the owner

Both are decision requests already written up with options and recommendations:

1. **Archive layout** (RFC-v0.21.3-002, Decision request 1) — whether the tarball
   keeps its internal `fjell-os-v{version}/` directory, which deviates from the
   project rules as written. Blocks an acceptance criterion, and therefore blocks
   the RFC-002 handoff.
2. **Gate 10 at the release cut** — Verus is absent in the implementer's
   environment, and the v0 exit criteria make a gate that cannot run a failure
   rather than an abstention. Either the release record is produced where Verus is
   installed, or v0.21.3 is tagged with a written accepted-risk statement. Not an
   architect call.

### v0.22 candidate themes

Accumulated across this release line, none yet scoped into an RFC:

| Candidate | Origin |
|---|---|
| Disposition of the 9 declared-but-undispatched syscalls | RFC-v0.21.3-001 §Deferred; ERRATA E-011 |
| Build-output non-determinism (Finding C, 9 of 28 binaries) | review-record-slice-2b-2c §4.3 — warrants its own RFC |
| ABI gate formatting sensitivity (line-based signature hashing) | review-record-slice-1-2 §4 |
| Mechanical syscall-count check | RFC-v0.21.3-001 testing requirement 8, declined as unauthorised gate surface |
| **Gates that pass while a documented rule goes unmet** | §3 above — four instances this line |

### Mandatory report to the owner

Per the governance policy's replanning trigger: **once v0.21.3 is cut and
RFC-v0.21.3-002 is implemented, no substantial development theme remains.** The
v0.22 candidates above are real but unscoped, and I must not turn them into a
roadmap unilaterally.

Joint planning is required to decide the v0.22 direction. This is a report, not a
proposal — the themes are listed as input to that discussion, not as a plan.
