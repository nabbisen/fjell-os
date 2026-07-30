# RFC-v0.22-001: Gate Integrity

**Status:** Proposed
**Milestone:** v0.22
**Tracks.** Verification-instrument quality. Cross-cutting; not a feature.
**Touches.** `crates/fjell-tools/src/callsite_audit.rs`,
`tools/fjell-abi-snapshot/`, `tests/abi/snapshot.json`, new check(s) under
`tools/`, `docs/rfcs/ERRATA.md`.
**Relates to:** RFC-v0.21.3-001 (which surfaced the pattern), RFC-v0.21.3-002
(the release cycle these gates serve), architect review H-03.

## Summary

Fjell settles every completion claim with eleven gates. RFC-v0.21.3-001 found
**four** separate instances of a gate reporting green while a documented rule
went unmet. That is one class of defect, not four bugs.

v0.22 makes the gates mean what they claim. It adds no OS functionality, changes
no security boundary, and touches no kernel, ABI, or crypto behaviour.

## Motivation

### The pattern, with its four instances

| # | Instance | Gate said | Reality |
|---|---|---|---|
| 1 | Reproducibility tier | PASS | The baseline was absent, and a missing baseline auto-recorded itself and returned success. The tier **could not fail**. |
| 2 | Gate 7, ERRATA register | PASS (0 OPEN) | E-011 was `ACCEPTED` but absent from the limitations document, which the release checklist requires. |
| 3 | Gate 4, ABI snapshot | PASS | Two `STABLE_CRATES` paths were stale since the v0.21.0 reorg; the scanner silently saw zero items for two crates. |
| 4 | Handoff status inheritance | n/a | A handoff's status stayed `Proposed` after its RFC moved to `Implemented` — RFC 000's named "status fields that lie". |

Each was found by human review, not by a gate. That is the problem: **the
instruments that certify this project's claims are themselves uncertified.**

### Gate 11 is substring matching

`callsite_audit.rs` decides capability enforcement is intact with:

```rust
let has_subset  = src.contains("is_subset_of");
let suspicious  = code.contains("new_rights &") && !code.contains("is_subset_of");
```

`contains` on file text. The check passes if the string occurs anywhere —
including inside a comment, a test, or a doc-string. It is labelled a "static
heuristic guard", which is honest, but it occupies a slot in an eleven-gate
table where readers reasonably infer something stronger. Architect review H-03
already called for a function-body scan; this RFC executes that.

### Gate 4 is formatting-sensitive by construction

The ABI signature is `simple_hash(trimmed_declaration_line)`. A whole-tree
`cargo fmt` therefore invalidates the baseline wholesale — as it did at
v0.21.3, producing 163 "changed signatures" that were entirely cosmetic. The
gate is line-based deliberately (`docs/src/abi/policy.md`) to avoid a
nightly-toolchain dependency, so this is a real trade-off, not a bug. But the
consequence is that a genuine signature change can hide inside formatting
churn, and the only reason none did at v0.21.3 is that a reviewer proved it
separately.

## The governing principle

**Every gate added or strengthened in this line must be demonstrated failing on
a deliberately broken input before it is accepted.**

A gate never observed to fail is a gate with no evidence it works. All four
instances above shared exactly that property. A test that proves the gate can
fail is therefore a required deliverable per item — not optional coverage.

## Goals

1. Gate 11 detects what it claims to detect, not the presence of a substring.
2. Gate 4 survives formatting changes without losing its ability to detect a
   real signature change.
3. Documented-surface drift (the RFC-v0.21.3-001 §M2 class) is caught
   mechanically rather than by review.
4. Where a documented rule is cheap to bind to a gate, it is bound.
5. Every gate touched here has a demonstrated failure mode.

## Non-goals

- No new OS functionality; no kernel, ABI, capability, lease, IPC, or crypto
  behaviour change.
- Not rewriting the gate harness or the release-rehearsal structure.
- Not resolving the 9 declared-but-undispatched syscalls — that disposition is
  still open and is not decided here.
- Not the negative-coverage completion (store/upgrade/svc), build determinism,
  or DMA unmap. Separately tracked.
- Not a v1.0 checklist executability audit. Owner cut this from scope
  (2026-07-30) since v1.0 is not in view; the Step 9 finding is recorded in
  ERRATA instead — see §Scope item 5.

## Scope

| # | Item | Size | Note |
|---|---|---|---|
| 1 | Mechanical syscall-count check | Small | Declined in v0.21.3 as unauthorised gate surface; authorised here. Compares the `fjell-abi` enum against the dispatcher's match arms and against the documented count. |
| 2 | Gate 4 signature normalisation | Small–medium | Normalise whitespace before hashing so reflow is inert. Requires a one-time baseline regeneration under the v0.21.3 Slice 2c provenance discipline. |
| 3 | Gate 11 → function-body scan | **Largest** | Replace `contains` over file text with a scan that establishes the token appears in the relevant function body, not anywhere in the file. |
| 4 | Bind documented rules to gates | Medium | At minimum: ACCEPTED errata must appear in the limitations document; RFC folder must agree with its `Status` field; a handoff's status must match its governing RFC. |
| 5 | Record the v1.0 checklist Step 9 finding in ERRATA | Trivial | `target/release-bundles/*.bundle` appears nowhere in `crates/` or `tools/`; Steps 9–10 are the signing steps. Record only — do not investigate or fix. |

Sequencing: 1 → 2 → 3 → 4, with 5 foldable anywhere. Item 1 first because it is
small and independent and proves the governing principle cheaply. Item 2 before
any future whole-tree `fmt`. Item 3 last of the substantive three because it is
the largest and most likely to surface findings.

## Risks

| ID | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R1 | A real Gate 11 exposes violations the weak check concealed | **Medium–high** | Medium | This is the intended outcome. Report findings; do **not** fix them in the same slice — that would repeat the scope-creep the v0.21.3 line avoided. |
| R2 | Changing the ABI signature algorithm invalidates the baseline | Certain | Low | One-time regeneration, with the pre-change measurement recorded so the diff's provenance stays recoverable. Same discipline as v0.21.3 Slice 2c. |
| R3 | A newly added gate is itself weak, compounding the problem | Medium | High | The governing principle: no gate is accepted without a demonstrated failure. |
| R4 | Rule-binding (item 4) grows without bound — many rules could be mechanised | Medium | Medium | Bind only rules already violated in practice. The three named in item 4 each have a recorded instance. |

## Testing and verification requirements

Per item:

1. A test that the check **fails** on a deliberately mismatched syscall count.
2. A test that a pure-reflow change produces **no** signature change, and that a
   genuine signature change still **does**.
3. A test that Gate 11 fails when the required token appears only in a comment
   or an unrelated function — the exact hole in the current implementation.
4. For each bound rule, a test that the gate fails when the rule is violated.

Plus the standing release-cycle exit criteria (RFC-v0.21.3-002).

## Acceptance criteria

- [ ] Every item in §Scope is implemented or explicitly deferred with a reason.
- [ ] **Each gate touched has a committed test demonstrating it can fail.**
- [ ] Gate 11 fails on a token present only in a comment.
- [ ] A whole-tree `cargo fmt` produces zero ABI signature changes.
- [ ] The syscall-count check fails on a deliberate mismatch.
- [ ] `cargo xtask release-rehearsal` reports all mechanical gates passing.
- [ ] Any violation surfaced by a strengthened gate is **reported, not silently
      fixed**, and dispositioned explicitly.
- [ ] ERRATA carries the Step 9 finding.
- [ ] Release record for `0.22.0` per RFC-v0.21.3-002.

## Alternatives considered

| Option | Assessment |
|---|---|
| **Strengthen the existing gates** *(chosen)* | Keeps the eleven-gate structure readers already know; fixes the instruments in place. |
| Replace the gate harness | Larger, riskier, and the harness structure is not the defect — the individual checks are. |
| Accept the gates as heuristics and rely on review | This is the status quo, and it produced four instances. Review caught them, but only because someone looked; that does not scale and is not repeatable. |
| Do function work first, defer this | Considered and put to the owner directly. Decision (2026-07-30): gates first, because every advancement theme will be certified by these same instruments. |

## Open question

Not blocking: whether Gate 11's scan should ultimately become AST-based rather
than a smarter textual scan. AST would be stronger but pulls in a parsing
dependency the project has so far avoided in its tooling. Recommend the textual
function-body scan now, and revisit only if it proves insufficient in practice.
