# RFC 026: Negative test harness

**RFC ID:** 026  
**Also known as:** RFC-v0.1.x-003  
**Status:** Proposed  
**Target version:** v0.1.2  
**Affects:** `tests/qemu/negative/`, service crates, `fjell-tools`

## Problem

Positive smoke tests (`TEST:Mx:PASS`) prove that happy paths work.
They do not prove that security boundaries are enforced.  A regression
that silently allows an unauthorised IPC send, a missing capability
check, or an out-of-range MMIO mapping would not be caught by any
existing test.

Fjell OS is a security-oriented project; negative results must be
first-class artefacts.

## Proposed fix

Add a *negative test harness* — a small framework on top of the
existing service / IPC infrastructure that:

1. **Sets up** a known starting state in a dedicated QEMU run.
2. **Attempts an invalid operation** through the normal syscall / IPC
   path.
3. **Verifies rejection** via the syscall return code or audit event.
4. **Verifies the audit marker** is recorded.
5. **Verifies the system state did not change unsafely** (e.g. the
   target region was not mapped, the persistent store did not change).
6. **Prints `NEG:<CATEGORY>:<CASE>:PASS`** to the serial console.

Each test is invoked by name through `cargo xtask qemu-negative
<category>` (RFC 025), which loads a QEMU profile, runs it, and asserts
the expected `NEG:*:PASS` markers.

## Required categories and cases

### Capability

```
NEG:CAP:INVALID_HANDLE:PASS
NEG:CAP:GENERATION_MISMATCH:PASS
NEG:CAP:MISSING_RIGHT:PASS
NEG:CAP:REVOKED_LEASE:PASS
NEG:CAP:DROPPED_HANDLE:PASS
```

### IPC

```
NEG:IPC:SEND_WITHOUT_RIGHT:PASS
NEG:IPC:CALL_WITHOUT_RIGHT:PASS
NEG:IPC:REPLY_INVALID_CALL_ID:PASS
NEG:IPC:REPLY_AFTER_REVOKE:PASS
NEG:IPC:BLOCKED_CALL_WAKES_ON_REVOKE:PASS
```

### MMIO

```
NEG:MMIO:MAP_WITHOUT_CAP:PASS
NEG:MMIO:MAP_RAM_REJECTED:PASS
NEG:MMIO:OFFSET_OUT_OF_RANGE:PASS
NEG:MMIO:REVOKED_REGION_REJECTED:PASS
```

### DMA

```
NEG:DMA:ALLOC_WITHOUT_CAP:PASS
NEG:DMA:SIZE_TOO_LARGE:PASS
NEG:DMA:REVOKED_REGION_REJECTED:PASS
NEG:DMA:ZEROIZED_ON_EXIT:PASS
NEG:DMA:QUARANTINE_TIMEOUT:PASS
```

### Store

```
NEG:STORE:CORRUPT_RECORD_REJECTED:PASS
NEG:STORE:PARTIAL_TAIL_IGNORED:PASS
NEG:STORE:BAD_SUPERBLOCK_MIRROR_REJECTED:PASS
NEG:STORE:VALID_PREFIX_RECOVERED:PASS
```

### Upgrade

```
NEG:UPGRADE:UNSIGNED_RELEASE_REJECTED:PASS
NEG:UPGRADE:INVALID_SIGNATURE_REJECTED:PASS
NEG:UPGRADE:ACTIVE_SLOT_WRITE_REJECTED:PASS
NEG:UPGRADE:HEALTH_FAILURE_NOT_CONFIRMED:PASS
```

Additional categories — RootFS, Policy, Recovery — are added under the
same pattern as the corresponding boundaries land in v0.2.

## Rationale

Markers are chosen for **deterministic prefix matching** so that the
existing `qemu-log-check` helper (RFC 025) needs no extra parsing.
Every case has the same shape: `NEG:<CATEGORY>:<CASE>:PASS`.

Each test exercises *one* invariant.  Combining cases (“send without
right *and* revoked lease in one test”) would obscure which boundary
caught the failure.

## Impact

- New `tests/qemu/negative/` directory with per-category profile
  files.
- One or more dedicated negative-test service crates may be added (e.g.
  `fjell-neg-cap`, `fjell-neg-ipc`) that try the invalid operations.
  These are not part of the production service plane.
- No kernel API changes; the harness exercises existing rejection
  paths.

## Test plan

- For each category listed above, at least one case exists.
- Running the corresponding `cargo xtask qemu-negative <category>`
  produces every listed `NEG:*:PASS` marker.
- A regression that allows the invalid operation must cause the
  corresponding test to either *not* print its marker, or to print a
  distinct `NEG:*:FAIL` marker — both of which fail the CI job.
- All negative tests are documented under
  `docs/src/development/negative-tests.md` (RFC 027 / 028 link).

## Implementation notes

- No fuzzing in v0.1.x.  No property testing.  No model checking.
  Those are deferred to v0.6 (per the v0.1-to-v0.9 roadmap).
- Cases that the current implementation cannot meaningfully test (e.g.
  `NEG:DMA:QUARANTINE_TIMEOUT:PASS` before quarantine is implemented)
  must emit `NEG:<CATEGORY>:<CASE>:DEFERRED` so the gap is visible in
  the log without failing CI.
- The harness must run with the same disk-image scaffold the smoke
  tests use; negative tests do not need their own kernel.
