# RFC 042: v0.2 negative test expansion

**RFC ID:** 042  
**Also known as:** RFC-v0.2-012  
**Status:** Proposed  
**Target version:** v0.2.0  
**Phase:** Phase 9 — Negative Test Completion  
**Related epics:** G (Negative Test Expansion)

## Problem

RFC 026 (v0.1.x-003) defined the negative-test harness with
categories `capability`, `ipc`, `mmio`, `dma`, `store`, `upgrade`.
v0.2 adds new categories that did not exist or were not enforced
in v0.1.x:

- `lease` — revoked-lease rejection (RFC 033)
- `user-copy` — invalid-pointer rejection (RFC 039)
- `audit` — drain authorisation and dropped count (RFC 039)
- `policy` — cap-broker bootstrap and default-deny (RFC 040)
- `evidence` — failure visibility (RFC 041)
- `svc` — service start, fault, quantum violations (RFC 037, 038)

Without expansion, regressions in these new boundaries would not
fail CI.

## Proposed fix

### CI matrix expansion

`cargo xtask qemu-negative <category>` gains the following
categories (all of which must run in CI):

```
cargo xtask qemu-negative capability
cargo xtask qemu-negative lease
cargo xtask qemu-negative ipc
cargo xtask qemu-negative mmio
cargo xtask qemu-negative dma
cargo xtask qemu-negative user-copy
cargo xtask qemu-negative audit
cargo xtask qemu-negative policy
cargo xtask qemu-negative store
cargo xtask qemu-negative upgrade
cargo xtask qemu-negative evidence
cargo xtask qemu-negative svc
```

### Required markers

Every marker introduced by RFCs 031–041 is included in CI:

- All `NEG:CAP:*` from RFCs 031, 032.
- All `NEG:LEASE:*` from RFC 033.
- All `NEG:IPC:*` from RFC 034 plus RFC 026.
- All `NEG:MMIO:*` from RFC 035.
- All `NEG:DMA:*` from RFC 036.
- All `NEG:SVC:*` from RFCs 037, 038.
- All `NEG:USER_COPY:*` and `NEG:AUDIT:*` from RFC 039.
- All `NEG:POLICY:*` from RFC 040.
- All `NEG:EVIDENCE:*` from RFC 041.

### Marker-based log checks

`cargo xtask qemu-log-check <log-file> <marker>` (RFC 025) is the
only validator used; no per-test custom matcher is allowed.  All
markers conform to the `NEG:<CATEGORY>:<CASE>:PASS` shape.

### Failure mode

A regression that allows an invalid operation must produce one of:

- no `NEG:*:PASS` marker (timeout → fail),
- `NEG:<CATEGORY>:<CASE>:FAIL` (explicit failure marker — fail).

Either fails the CI job.  No silent skip is allowed.

## Rationale

Negative tests are the only evidence that boundaries work *for the
attacker’s side of the contract*.  Positive smoke tests
(`TEST:Mx:PASS`) prove the happy path; without negative tests
covering the same boundary, a regression that breaks the rejection
path is invisible.

A uniform marker shape lets the existing `qemu-log-check` validate
every category; custom matchers would invite parser drift.

## Impact

- Crates: `fjell-tools` (no API change — adds profiles), one or
  more negative-test service crates per category if they grow large
  enough to warrant separation.
- New profile files under `tests/qemu/profiles/<category>.toml`.
- New documentation: `docs/development/negative-tests.md` lists
  every marker and links to the RFC that introduced it.

## Test plan

- Each marker listed above corresponds to a profile in
  `tests/qemu/profiles/`.
- Running the corresponding `cargo xtask qemu-negative <category>`
  produces every listed marker.
- A "kill switch" sanity check: temporarily disable one
  enforcement check; confirm the corresponding negative test fails
  (then revert).  This protects against tests that always pass.

## Implementation notes

- Out of scope: fuzzing harness, property-test framework adoption,
  model checking — all v0.6 work.
- The "kill switch" sanity check should be performed at least once
  per category before tagging v0.2.0.  Automating it (e.g. via a
  feature flag that intentionally weakens a check) is desirable but
  not required for v0.2.
- Markers labelled `NEG:<C>:<CASE>:DEFERRED` are allowed only for
  cases whose RFC has explicitly deferred them (e.g. quarantine
  timeout cases before RFC 036 lands).  DEFERRED counts as PASS
  for CI; it must be removed before the relevant RFC closes.
