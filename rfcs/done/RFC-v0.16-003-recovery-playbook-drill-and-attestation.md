# RFC-v0.16-003: Recovery Playbook Drill and Attestation

**Status:** Implemented (v0.16.0)
**Milestone:** v0.16 — Validation Closure
**Addresses:** architect review RB-04; errata E-005, E-008

## Problem

The recovery guide existed but was never walked through by a non-author,
and the follow-test attestation required by RFC-v0.15-004 §3 was missing.

## Change

Walked the recovery guide against the reference environment and committed
`docs/operations/recovery-drills/v0.16-dr-walkthrough.md`. Scenarios
exercised against real crate APIs (not just read):

- **DR1 Coordinator loss** — `CoordinatorPromotion`, operator-signed path.
- **DR2 Key compromise** — `RevocationTable` FSM + re-sign + verify with
  the now-encrypted key (RFC-v0.16-006).
- **DR5 Audit corruption** — `fjell-summary-check` catches seq regression.
- **Partition handling** — covered by the RFC-v0.16-002 drill.
- **Boot triage** — `fjell-dtb-validate` returns the documented R4 code.

## Honesty

DR3, DR4, DR6, DR7, DR8 remain document-only and are NOT claimed as
drilled. They are listed in the v1.0 limitations (RFC-v0.16-005).

## Test plan

The attestation references reproducible markers from the partition drill
and the summary checker. The DR2/DR1/boot arms are API-level walkthroughs
recorded in the attestation document.
