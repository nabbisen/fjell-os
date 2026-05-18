# RFC 059: v0.2.12 release-gate criteria

**RFC ID:** 059
**Also known as:** RFC-v0.2-025
**Status:** Proposed
**Target version:** v0.2.12
**Phase:** Service separation + release-gate close
**Supersedes:** RFC 043 (original v0.2 release gate)
**Depends on:** all of RFCs 048-058

## Problem

RFC 043 ("v0.2 Security Boundary Release Gate") defined a marker-count
checklist for v0.2.0 closure.  Two problems emerged in practice:

1. The v0.2.8 review demonstrated that marker counts alone are
   insufficient — a marker can pass for the wrong reason (`is_err()`
   without checking error code).  RFC 050 addresses the test-side fix,
   but the gate document needs to require *exact-error verification* for
   pass-rate to be meaningful.

2. RFC 043 listed "21 markers" as the bar, but RFCs 048-058 add new
   markers and recharacterize existing ones.  The bar moves; the gate
   document needs to enumerate the v0.2.12 final list, not the v0.2.8
   intermediate list.

RFC 059 supersedes RFC 043 with concrete, verifiable criteria.

## Proposed fix

### Pre-conditions for granting `TEST:V02:PASS`

1. **RFC status**: all of RFCs 031-058 are in `Implemented` state.
   Implementation = code merged, tests pass, CHANGELOG entry filed.

2. **Host tests**: `cargo test` passes on every host-buildable crate.
   Count baseline (v0.2.8): ~86 tests.  Final count subject to additions
   per RFC test plans.

3. **QEMU negative-test matrix**: every marker in the table below
   emits `PASS` in its QEMU profile run.  No `NEG:HARNESS:WRONG_ERROR:*`
   or `NEG:HARNESS:UNEXPECTED_OK:*` diagnostic lines (per RFC 050).

4. **CI matrix**: the GitHub Actions workflow runs every category in the
   table below with no failures across 3 consecutive runs.

5. **Smoke test**: `cargo xtask qemu-test m8` passes (existing).

6. **Documentation**: README, ROADMAP, CHANGELOG, release-gate doc all
   describe v0.2 as complete.  No "Partial" or "Not complete" rows
   remain in the ROADMAP's v0.2 phase table.

### Final marker matrix (post-v0.2.12)

| Category | Markers | Total |
|----------|---------|-------|
| **capability** | WRONG_KIND_REJECTED, RIGHTS_DENIED, LEASE_REVOKED, DROP_ON_REVOKED (4 existing) + WRONG_SCOPE_REJECTED, STALE_GENERATION_REJECTED (RFC 048) + COPY/MINT/REVOKE/INSPECT_WITHOUT_RIGHT_REJECTED (RFC 049) | 10 |
| **mmio** | RIGHTS_CHECK, BOUNDS_REJECTED, RAM_GUARD_REJECTS (3 existing) + SCOPE_MISMATCH_REJECTED, ALREADY_MAPPED_REJECTED, DEVICE_VMA_EXHAUSTED (RFC 051) | 6 |
| **dma** | RIGHTS_CHECK, REVOKE_EXPLICIT, ZEROIZE_ON_EXIT (3 existing) + REGION_TABLE_FULL_ROLLBACK, REVOKE_WITHOUT_RIGHT_REJECTED, REVOKE_UNMAPS_VA (RFC 052) | 6 |
| **user-copy** | NULL_REJECTED, KERNEL_ADDR_REJECTED (2 existing) — now exact-error checked | 2 |
| **policy** | DEFAULT_DENY, BOOTSTRAP_GUARD, DENY_PRIORITY (3 existing) + IDENTITY_SPOOFING_REJECTED (RFC 055) + GRANT_INSTALLS_USABLE_CAP (RFC 056) | 5 |
| **audit** | EVIDENCE_GAP_DETECTED (1 existing) + LEASE_REVOKED_REJECTED (RFC 054, if spawn lease binding lands) | 1-2 |
| **ipc** | BLOCKED_RECV/CALL_WAKES_ON_REVOKE, LATE_REPLY_REJECTED (3 existing) — now exact-error checked | 3 |
| **svc** | START_TIMEOUT_DETECTED, FAULT_DETECTED (2 existing) + READY_ACCEPTED, UNAUTHORIZED_READY_REJECTED (RFC 058) | 4 |
| **bootctl** | UNAUTHORIZED_REBOOT_REJECTED, CONFIRM_WITHOUT_PENDING_REJECTED, BOOTCTL_DOWN_BLOCKS_CONFIRMATION (RFC 057) | 3 |
| **cap_install** | WITHOUT_AUTHORITY_REJECTED (RFC 056) | 1 |
| **harness** | CSpace_LAYOUT_VALID (RFC 050) | 1 |

**Total: 42 markers** (vs 21 at v0.2.8 — though v0.2.8's 21 may be unreliable
per the review).

### Evidence package for the gate

When the gate is claimed, the following must be archived:

1. CI run URLs for the 3 confirming runs.
2. QEMU serial logs for each of the 11 categories (saved as artifacts in
   `tests/qemu/artifacts/`).
3. The cargo test output showing all host tests passing.
4. The release tarball: `fjell-os-0_2_12.tar.gz`.
5. This RFC marked `Implemented` with the actual marker counts observed.

### Out-of-scope for v0.2 (deferred to v0.3)

| Item | Reason |
|------|--------|
| DMA quarantine timeout (H-03) | Timer infrastructure into userspace = v0.3 |
| cap-broker delegation log retention/pruning | Storage policy = v0.3 |
| Multi-tenant audit partitioning | No use case yet |
| Service auto-restart on fault | RFC 058 detects; restart is v0.3 |
| Hardware-rooted trust (TPM/eFuse) | v0.3 theme |

These are explicitly documented as deferred, not silently omitted.

## Rationale

**Why supersede RFC 043 instead of amending?**  The review demonstrated
that RFC 043's checklist was operationally insufficient.  Marking it
superseded makes the change of bar explicit in the RFC history rather
than hiding behind an edit.

**Why 3 consecutive CI runs?**  Single-run flakes in cooperative-IPC
tests have historically occurred when scheduler ordering shifted.
Three consecutive successes raises confidence that the markers fire
deterministically.

**Why archive serial logs?**  Audit trail for the gate itself.
Re-reviewing v0.3 work will need to ground on "did v0.2 actually pass"
— logs make that verifiable.

**Why list deferred items explicitly?**  The review (H-03) called out
that deferred items must not be silently counted as complete.  This
RFC's explicit deferral table is the response.

## Impact

### Crates affected

None directly — this RFC is documentation/process.  All technical work
is in RFCs 048-058.

### Backward compatibility

RFC 043's gate token (`TEST:V02:PASS`) is preserved; only the
acceptance criteria change.

## Test plan

This RFC has no test plan in the conventional sense.  Its "test" is the
gate decision process itself: at v0.2.12 close, the release manager
walks the criteria and produces a `gate-decision-v0.2.12.md` document
recording the outcome.

A template for that document:

```markdown
# v0.2.12 Gate Decision

Date: <YYYY-MM-DD>
Decided by: <name>

## Pre-conditions
- [ ] RFCs 048-058 all Implemented (list)
- [ ] Host tests: <N> passing
- [ ] QEMU markers: <M>/42 PASS, 0 WRONG_ERROR, 0 UNEXPECTED_OK
- [ ] CI runs: <urls of 3 consecutive green runs>
- [ ] Smoke: m8 passes
- [ ] Docs reconciled

## Deferred (acknowledged)
<list per Out-of-Scope table>

## Decision
[ ] TEST:V02:PASS GRANTED   [ ] WITHHELD (reason)
```

## Implementation notes

- The `gate-decision-v0.2.12.md` file lives in `docs/src/releases/`.
- The release-gate doc at `docs/src/releases/v0.2.0-release-gate.md`
  should be replaced (or sub-divided into per-release files).
- RFC 043 should be moved to status `Superseded` with a pointer to
  this RFC.

## Open questions

- Should the 3-CI-run requirement be relaxed if the runs are
  deterministic by construction (which they are after RFC 050)?
  Recommendation: keep at 3 for v0.2.12 closure; relax to 1 once
  scheduler determinism is formally verified (v0.3+).
- Should there be a periodic re-validation (run the matrix monthly on
  main)?  Recommendation: yes, but out of scope for the gate document
  itself.
