# RFC-0.24-001: Instrument Audit — do the checks check what they claim?

**Status:** Implemented-with-Errata (0.24.0) — accepted 2026-08-02; see ERRATA E-017
**Milestone:** 0.24 — **confirmed.** The owner set the 0.24 direction to this
line on 2026-08-02, choosing trustworthiness over capability for this release.
**Tracks.** Verification-instrument integrity, systematically rather than
incidentally.
**Touches.** Audit-only in its first pass — no instrument is changed without a
separate disposition.
**Relates to:** RFC-v0.22-001 (strengthened four instruments it already
suspected; this asks which others are lying), RFC-v0.23-002, ERRATA E-013.

## Summary

This project has found **eleven** instances of an instrument reporting success
without having checked. Every one was found **incidentally** — while doing
something else. None was found by looking.

There are roughly **55 instruments**: 12 release-rehearsal gates, 19 `test-all`
tiers, 16 CI jobs, and 8 committed artifacts that assert repository state. They
have never been audited as a set.

This RFC audits them, against a taxonomy derived from the eleven known
instances, and requires each to be demonstrated failing.

## Motivation

### Eleven instances, none found deliberately

| Instance | Found while |
|---|---|
| Gate 11 satisfied by a substring in a comment | writing the v0.22 RFC |
| Gate 4 scanning stale paths, silently seeing zero items for two crates | reviewing a fmt pass |
| Gate 7 green over an `ACCEPTED` erratum absent from limitations | reviewing v0.22 slices |
| Handoff status still `Proposed` after its RFC shipped | a disposition sweep |
| Repro tier auto-recording a missing baseline and passing | reading the tool for another reason |
| `trust-report.txt` committed asserting `Version: 1.0.0` | preparing a release |
| Milestone markers keyed on task-table index, not identity | a latency change re-pointed an index |
| `m8` green while the M8 path never ran | investigating the above |
| `test-all` tier 1 silently skipping `fjell-kernel` entirely | trying to add a unit test |
| 13 broken relative links in tracked docs | verifying an unrelated file move |
| grep blindness ×3 (NUL padding, hex constants, UART interleaving) | three separate measurements |

Eleven for eleven, by accident. That is not a discovery process; it is luck with
good record-keeping.

### This is partly an audit of the review process

Most of these passed through architect review — mine — without being caught,
several of them repeatedly. The instruments were trusted because they were
green, which is precisely the assumption under examination. An audit that only
examines the tools and not the habit of believing them would miss half the
problem.

## The taxonomy

Five failure modes, each with at least two concrete instances already on record.
This is the value of the RFC: it converts "be careful" into something checkable.

**1. Scope blindness — the instrument does not examine what it claims to.**
`test-all` tier 1 runs `cargo test --workspace --lib`; `fjell-kernel` declares
only `[[bin]]`, so it is omitted with no error (E-013). Gate 4's
`STABLE_CRATES` held paths stale since the v0.21.0 reorg and saw zero items for
two crates. `grep` silently skips files its binary heuristic rejects.

**2. Proxy attestation — it checks something correlated, not the property.**
`TEST:V0.7-SYNC:PASS` attested that task index 19 exited, not that syncd
succeeded. `TEST:M8:PASS` attested upgraded's exit, not that the M8 path ran.

**3. Fail-open on absence — missing input reads as success.**
`repro-check --skip-build` recorded a baseline it could not find and returned
success. The shared TOML array parser closed the array at a `]` inside a string
and loaded 2 of 4 markers, silently.

**4. Weak predicate — satisfiable without the property holding.**
Gate 11 decided capability enforcement was intact via
`src.contains("is_subset_of")`, satisfiable by a comment.

**5. Stale assertion — a committed record asserts state that no longer holds.**
`trust-report.txt` at `Version: 1.0.0`. RFC-v0.17-001's status requesting a
decision made and shipped two lines earlier. Handoff statuses after their RFCs
moved. Thirteen broken doc links.

## The audit method

For each instrument, four questions:

1. **What does it claim?** In one sentence, as a reader would understand it.
2. **What does it actually examine?** From the code, not the label.
3. **Which modes could it exhibit?** Against the five above.
4. **Demonstrate it failing.** On a deliberately broken input.

Question 4 is the deliverable. RFC-v0.22-001 established that a gate never
observed to fail is a gate with no evidence it works; this applies that
standard to the instruments v0.22 did not touch.

## Scope

| Pass | Instruments | Count |
|---|---|---|
| 1 | `release-rehearsal` gates | 12 |
| 2 | `test-all` tiers | 19 |
| 3 | Committed state-asserting artifacts | 8 |
| 4 | CI jobs | 16 |

Ordered by release-criticality. A pass may be cut without invalidating earlier
ones — this is deliberately interruptible, because it is an audit and audits
expand.

**Committed artifacts** in pass 3: `trust-report.txt`, `v1-readiness.md`,
`abi/snapshot.json`, `repro/baseline-digests.txt`, `syscall/expected.toml`,
`ERRATA.md`, `v1-limitations.md`, `rfcs/README.md`. Each asserts something about
the repository; each can go stale silently.

## Non-goals

- **Not fixing what it finds.** Findings are reported and dispositioned
  individually. An audit that also repairs becomes unreviewable, and every line
  in this project that mixed the two nearly went unbounded.
- Not rewriting the gate or tier harness.
- Not adding instruments. This audits the ones that exist.
- Not E-013's fix — that is architectural (a `[lib]` target, or splitting a
  host-testable subset) and stays its own item. E-013 is mode 1's exemplar and
  its **finding** folds in here; its **fix** does not.

## Risks

| ID | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R1 | The audit finds a lot | **High** | Medium | That is the purpose. Report; disposition separately; do not fix in-pass. |
| R2 | Unbounded — 55 instruments deeply analysed is a large line | **High** | High | Timebox per instrument. The question is narrow: *what would make this pass without checking?* Passes are independently cuttable. |
| R3 | The audit is itself unverified — the meta-problem | Medium | High | For each instrument, the demonstrated failure **is** the verification. An instrument with no demonstration is recorded as unaudited, not as passing. |
| R4 | Findings are argued away as acceptable rather than recorded | Medium | High | Apply the test used for E-012: would this classification stand with no gate watching? |

## Acceptance criteria

- [ ] Every instrument in the attempted passes is recorded with its four
      answers.
- [ ] Every instrument claimed as sound has a **committed demonstration of it
      failing**.
- [ ] Instruments not audited are listed as **unaudited**, never as sound.
- [ ] Every finding is dispositioned explicitly — fixed, deferred with a
      reason, or accepted with the E-012 test applied.
- [ ] No instrument is modified by this RFC beyond what a demonstration
      requires.

## Open question for the owner

**Whether this is 0.24's theme at all.** It is scoped here because scoping it
was requested, not because the slot is claimed. It competes with the three
directions left open in the options paper — service plane, human operability,
hardware — and with the v0.23 candidate list.

My view, offered as input rather than as a plan: this is worth doing before the
next *functional* line, because every functional line is judged complete by
these same instruments, and eleven of them have now been caught not checking.
But that is a sequencing judgement, and the direction is the owner's.
