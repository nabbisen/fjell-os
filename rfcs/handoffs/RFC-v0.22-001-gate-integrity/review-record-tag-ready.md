# Review Record — 0.22.0 tag readiness, and RFC-v0.22-001 disposition

**Reviewer:** architect
**Reviewing:** `review-request-v0.22.0-tag-ready.md` (commit `e96e898`)
**Date:** 2026-07-31

## Outcome

**Approved. Release recommendation: tag `0.22.0`.**

RFC-v0.22-001 is dispositioned **Implemented (v0.22.0)** in the same commit
series, per the lifecycle policy.

One defect found and fixed by me before recommending the tag (§3).

## 1. Verified independently

| Claim | Result |
|---|---|
| `Cargo.toml` at `0.22.0` | **Confirmed** |
| CHANGELOG `## [0.22.0] — 2026-07-31 — Gate integrity` | **Confirmed**, version and date both present |
| Release record: BLOCKED marker gone, twelve-gate table | **Confirmed** |
| E-012 per-entry `Resolution:` now `ACCEPTED` | **Confirmed** |
| Twelve gates green | **Confirmed** — re-run by me at HEAD and again after disposition |

## 2. A correction they made to my work

Their §1.4: my disposition commit `50cb75d` updated E-012's Summary-table row
and the trailing paragraph to `ACCEPTED`, but left the erratum's own per-entry
`**Resolution:**` line reading `**OPEN**`. The document contradicted itself.

They fixed it, and were right to flag that no gate would have caught it —
`errata-limitations` reads only the Summary table. So a register whose summary
and body disagreed would have passed.

That is my error, and it is the same class this entire line is about: a
mechanical check green over an internal inconsistency it does not look at. Worth
recording that the architect produced one of these while reviewing the line that
exists to eliminate them.

**Not proposing a new subcheck for it.** R4 warns against unbounded rule
addition, and the general form — "every document is internally consistent" — is
not mechanisable at reasonable cost. Recorded as a known limit of Gate 12's
`errata-limitations`: it validates the summary against limitations, not the
register against itself.

## 3. Defect found before the tag — stale trust report

`docs/release/trust-report.txt` was committed reading:

```
Generated : 2026-06-05 05:11:23 UTC
Version   : 1.0.0
```

A June artifact from a v1.0 dry run, asserting a version this project has never
released, sitting in `docs/release/` through every release since.

It survived by a mechanism worth naming: **every gate run regenerates it, and
the convention has been to revert that as run noise** — including by me, twice
in this session. So the stale copy was never replaced. Gate 6 checks only that
six sections are present, so it passed throughout.

Regenerated at `0.22.0` and committed (`a09d66e`). Six sections preserved, so
Gate 6 is unaffected. **A release artifact should be regenerated and committed
by the release that produces it, not reverted** — the revert convention is
correct for a mid-line gate run and wrong at a release cut.

This one is neither the implementer's miss nor solely mine; it is a convention
that quietly produced a false artifact. Recorded so the next release does not
repeat it.

## 4. RFC-v0.22-001 disposition

Moved to `rfcs/done/` with `Status: Implemented (v0.22.0)`, reference sweep in
the same commit: release-record header, handoff governing link and inherited
status, and the index. Dated submissions keep their `proposed/` references per
the standing rule.

Verified after the sweep: index links all resolve; `done/` and the index
describe each other exactly in both directions (157 files); Gate 12
`rfc-status-folder` PASS across 156 RFCs — the gate that caught RFC-v0.17-001
now also guards this disposition, which is the first time an RFC move in this
project has been mechanically checked.

## 5. Release recommendation — `0.22.0`

| Item | Status |
|---|---|
| Entry criteria | 4/4 |
| Exit criteria | 8/8 |
| Mechanical gates | **12/12 PASS** |
| Gate 9 | Unsigned — v1.0-scoped, correctly out of scope |
| Accepted-risk statement | **None required** |
| Supersedes | `0.21.3` |

Delivered: Gate 11 from substring matching to a real function-body scan; ABI
signatures normalised so formatting churn cannot conceal a change; a
syscall-surface check; three documented rules bound to a gate; Gate 12 added.

Two real defects were caught by the new gates during their own implementation —
RFC-v0.17-001's status, which had lied for two release lines through multiple
reviews, and E-012's classification. That is the line demonstrating itself.

Remaining owner action: apply the tag.

## 6. Portfolio review — required at this disposition point

`rfcs/proposed/` is now **empty**. No active development theme remains.

**v0.23 candidates**, recorded and unscoped:

| Candidate | Origin |
|---|---|
| ACCEPTED is an unguarded escape hatch — Gate 7 enforces `0 OPEN` while ACCEPTED has no gate of its own | review-record-slices-1-4 §6 |
| The 9 declared-but-undispatched syscalls — disposition still undecided | RFC-v0.21.3-001 §Deferred; ERRATA E-011 |
| Build-output determinism (Finding C) | review-record-slice-2b-2c §4.3 |
| Negative-coverage completion — store/upgrade emitters, svc READY 2/4 | roadmap v1.1 list |
| DMA user-VA unmap | kernel debt since v0.8.x |
| v1.0 checklist executability — E-012 and whatever else an audit finds | ERRATA E-012 |

**The deferred direction discussion is now due.** The owner scheduled it for
v0.22 completion, which the tag completes. What is owed is an options paper —
per direction: what exists, what is genuinely missing, the dependency chain,
honest sizing, and what claim it would let the project make. Not a menu; the
owner explicitly declined to choose from short options, and was right to.

The measured finding that bears on it stands: `proxy-text` holds 845 lines of
working renderer that nothing calls, and 17 of 29 services never receive IPC.
Fjell's distinguishing demonstration is largely built and entirely unwired.
