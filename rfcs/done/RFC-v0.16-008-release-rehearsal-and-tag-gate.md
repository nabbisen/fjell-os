# RFC-v0.16-008: Release Rehearsal and v1.0 Tag Gate

**Status:** Implemented (v0.16.0)
**Milestone:** v0.16 — Validation Closure
**Addresses:** architect review (final tag gate D11)

## Problem

The v1.0 tag must be earned by a rehearsed, gated process, not applied ad
hoc. The earlier premature v1.0.0 tag (reverted) demonstrated the risk.

## Change

The release checklist (RFC-v0.15-003) gains an errata gate and a drill
gate. The v1.0 tag gate requires ALL of:

1. Host tests pass (0 failures).
2. Unsafe audit: 0 missing.
3. MMIO audit: 0 missing.
4. ABI snapshot: verify PASS.
5. Readiness matrix: 0 OPEN.
6. Trust report: 6 sections populate.
7. **ERRATA register: 0 OPEN** (new — RFC-v0.16-004).
8. **Validation drills pass** (new): Ed25519 TV1, partition reconcile,
   rollback rejection, SDK runtime, SDK convergence.
9. Release notes carry the v1.0 limitations section (RFC-v0.16-005).

## Tag gate decision

The v1.0.0 tag remains **owner/architect-gated**. v0.16 closure produces
the evidence; the tag itself is applied only on explicit approval.

## Test plan

`cargo xtask release-rehearsal` runs gates 1–8 and prints a PASS/FAIL
matrix. Gate 9 is a human checklist item.
