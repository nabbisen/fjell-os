# Review Record — RFC-v0.21.3-001, Slice 3

**Reviewer:** architect
**Commits reviewed:** `de2a74d` (§4.1), `14ea16b` (§4.2), `9e521e3` (§4.3), `f3ad586` (CHANGELOG)
**Date:** 2026-07-30

## Outcome

**Conditionally Approved.** One correction required (§4), otherwise accepted.

Reviewed without a submitted review request — the commits were pushed directly.
That is a process deviation, noted in §6, but the work itself is the strongest
of the three slices.

## 1. Verified independently

| Claim | Method | Result |
|---|---|---|
| No surviving "38 syscalls" claim | `grep -rn "38 syscall" docs/` | **Confirmed** — none |
| `IpcTrySend` corrected in the normative ABI doc | grep | **Confirmed** — now states no distinct number exists; wrapper issues `IpcSend` (20) |
| Syscall surface documented as 35 declared / 26 dispatched / 9 not | read `kernel.md` §2 | **Confirmed**, with an explicit "Declared, not dispatched" section |
| SUMMARY.md and rfcs/README.md links resolve | mechanical link check of all relative links | **Confirmed** — 0 broken |
| Version stamps agree | README badge, README prose, `Cargo.toml` | **Confirmed** — 0.21.3 in all three |
| Empty doc dirs removed | filesystem | **Confirmed** — both gone |
| Gate 7 still green | `grep -cE "\| *OPEN *\|"` on ERRATA | **Confirmed** — 0 OPEN |

### §4.3 tested empirically, not read

All four paths of the fail-closed change, with exit codes measured directly
(my first attempt piped through `tail` and read `tail`'s status — corrected):

| Scenario | Expected | Actual |
|---|---|---|
| Baseline present, digests match | pass | exit 0, `PASS (28 artefacts identical)` |
| Baseline missing, no flag | **fail closed** | exit 1, `FAIL — no baseline … (this is not a passing state)` |
| Baseline missing, `--record-baseline` | record, pass | exit 0, baseline written (29 lines) |
| Baseline **present**, `--record-baseline` | refuse | exit 1, refuses to overwrite |

The fourth behaviour was not something I specified. Refusing to silently
re-record over an existing baseline is a correct addition — it closes the
obvious way to launder a failing check into a passing one.

They also found and fixed a real bug I had not seen: `cargo xtask repro-check`
dropped all extra arguments, so `--record-baseline` could never have reached
the tool. Without that, the feature would have shipped unreachable.

## 2. Finding C is resolved in the favourable direction

The two-build check was run — the experiment I required before Finding C could
be filed:

> **PASS, 29 artefacts identical, in one environment.**

So the phenomenon is **cross-environment only**. The reproducible-build NFR
holds within an environment, as documented. That is the lower-severity branch of
the two hypotheses.

**Consequence: the escalation I flagged does not fire.** I had said that if the
two-build check failed, the v1.0 limitations and trust-report wording would need
owner review. It passed, so no owner action is required and no claim needs
softening. Finding C stands as a genuine but bounded issue, correctly deferred
to its own v0.22 RFC.

This is the value of insisting on the experiment rather than accepting the
inference: the earlier characterization pointed at within-environment
non-determinism, which would have been considerably worse.

## 3. Judgement calls accepted

- Filing the `cap_install` discrepancy as **ERRATA E-011** rather than inventing
  a new register. Correct — ERRATA is the drift register and this is drift.
- Marking it **ACCEPTED** rather than OPEN. Correct: the path fails closed
  (`UnknownSyscall`), so it is a disclosed limitation, not a live hole, and
  OPEN would have failed Gate 7 for something already governed by an RFC.
- Recording the ABI gate's formatting sensitivity and the 9 undispatched
  syscalls in the CHANGELOG's "Known limitations" section rather than burying
  them in prose.

## 4. Correction required — E-011 is ACCEPTED but absent from the limitations doc

`docs/release/release-checklist.md` Step 7b states:

> ACCEPTED items are permitted but must each appear in the release notes
> limitations section.

`docs/release/v1-limitations.md` — the authoritative Gate 9 list — references
**E-004 only**. E-011 is not there. The rule is otherwise followed: E-004 is
present as item 1, so this is a genuine omission rather than a dead rule.

ERRATA's own summary asserts the ACCEPTED items "are reflected in the v1.0 scope
statement / RFC-v0.21.3-001". For E-004 that holds. For E-011 it points at an
RFC, not at the limitations document the checklist actually names.

**Required:** add E-011 to `docs/release/v1-limitations.md` as a numbered item,
governed record `ERRATA E-011` / `RFC-v0.21.3-001 §M2`. Wording should state
that `cap_install`'s documented rights validation does not execute because the
syscall is not dispatched, that the path fails closed, and that disposition is
deferred to v0.22.

Why this matters beyond bookkeeping: **Gate 7 passes mechanically while a
documented rule is unmet.** A green gate concealing an unmet requirement is
precisely the failure class this release line exists to remove. It is also the
second instance of the same shape — the first was the repro tier that could not
fail. Worth noting as a pattern rather than a one-off.

## 5. Carried forward, not faults

- **`0.21.2` is not yet marked `KNOWN-BAD`.** The owner accepted that decision
  *after* these commits. Outstanding work under RFC-v0.21.3-002, not a defect
  in Slice 3.
- `crates/fjell-kernel/prebuilt/fjell-neg-test.bin` changed inside the
  documentation commit `14ea16b`, alongside the `0.21.2 → 0.21.3` version bump.
  Almost certainly the embedded version string, and the baseline recorded in
  the later `9e521e3` matches it, so the tier is consistent. Confirm the cause
  in the release record rather than leaving it inferred.

## 6. Process note

No review request was submitted for Slice 3; the commits were pushed and I
reviewed them directly. Two consequences worth stating:

1. The evidence I needed — the two-build result — was recoverable only because
   it happened to be written into the CHANGELOG. That was luck, not process.
2. The §7 review-request format exists so the implementer states known
   limitations and requested review focus. Had E-011's limitations-doc gap been
   surfaced there, it would have been caught before review.

Submit the request for the remaining work.

## 7. Required next deliverables

1. E-011 added to `docs/release/v1-limitations.md` (§4).
2. `0.21.2` marked `KNOWN-BAD` in `CHANGELOG.md` (owner-accepted).
3. A review request covering both.

After that, RFC-v0.21.3-001 is complete and v0.21.3 becomes the first release
cut under RFC-v0.21.3-002 — which requires a release record with real gate
output, including an explicit accepted-risk statement for Gate 10 if Verus
remains unavailable.
