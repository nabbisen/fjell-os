# RFC-v0.13-001 — Fleet Reliability and Recovery Depth Overview

**Status:** Proposed
**Target version:** v0.13.0
**Parent:** RFC 061 §10.
**Cross-refs:** v0.13-002 through v0.13-005.

## 1. Purpose

v0.8 shipped the *types* of fleet operations — roster, rollout plan,
policy distribution, diagnostics, recovery intent. v0.10-005 will
demonstrate them on three nodes. v0.11 closes the trust spine.
v0.12 reaches real hardware. v0.13 is the milestone where the fleet
becomes *operationally honest*: it must continue to behave correctly
under partial failure, key compromise, partition, and operator
mistake.

The bar for "operationally honest" is concrete:

- A partitioned fleet reconverges without losing or fabricating
  evidence.
- A compromised key can be retired and replaced without taking the
  fleet offline.
- A staged rollout that fails on N% of nodes does the right thing —
  bounded, observable, recoverable.
- An operator with a documented playbook can recover from any
  catalogued failure without ad-hoc reasoning.

## 2. Composition

| RFC | Title | Deliverable |
|-----|-------|-------------|
| v0.13-001 | This overview | Coordination |
| v0.13-002 | Fleet Split, Reconnect, and Reconciliation | Partition-aware sync state machine |
| v0.13-003 | Key Compromise Recovery Playbook | Operator playbook + automation |
| v0.13-004 | Bulk Re-attestation and Staged Rollout Failure Handling | Throttled re-attest + rollback semantics |
| v0.13-005 | Disaster Recovery Patterns and Semantic Summary Consistency | DR runbook + consistency checks |

## 3. Posture

v0.13 *does not* introduce a fleet dashboard, a web UI, or any new
remote-control authority. Operability here means written procedures
backed by mechanical enforcement, not a graphical interface.

Three discipline rules apply across all v0.13 sub-RFCs:

- **No silent failure.** Every recovery path emits semantic evidence;
  no path "just works without trace."
- **Bounded blast radius.** Every operation has an explicit upper
  bound on how many nodes it can affect before pausing for
  confirmation.
- **Operator > automation.** Where a choice exists between automated
  recovery and human confirmation, the default is human confirmation
  with the automation queued and signed.

## 4. What v0.13 explicitly does *not* include

- New transport protocols (the v0.4 stack remains authoritative).
- Real-time guarantees. Fjell is not an RTOS even after v0.13.
- A dashboard (web, TUI, or otherwise). Operator views are file +
  Trust Report.
- Multi-fleet federation. Single-fleet only; multi-fleet topology is
  v1.x territory.
- Self-healing without operator approval, except for documented
  bounded paths (e.g. retry of a single failed measurement upload).

## 5. Release criteria

v0.13.0 may be tagged when:

1. The four sub-RFCs are merged to `done/`.
2. A partition-recovery scenario in the reference fleet (RFC-v0.10-005)
   produces convergent state with full evidence preservation.
3. The key-compromise playbook (v0.13-003) runs end-to-end against
   the reference fleet, rotating an active key without operator
   downtime.
4. Bulk re-attestation completes for the reference fleet within its
   documented throttle bounds and produces a complete result manifest.
5. Each documented failure mode in the disaster-recovery runbook
   (v0.13-005) has a fixture that exercises it.
6. The Trust Report gains a "Recovery posture" section listing
   recoverable failure modes and the most recent invocation per mode.

## 6. Risk register

| Risk | Mitigation |
|------|------------|
| Recovery procedures become aspirational vs. tested | Every playbook step has a CI fixture |
| Operator playbook drifts from code | Playbook excerpts run as `bash run-verified` blocks (RFC-v0.10-006) |
| Bulk re-attestation overwhelms a small fleet | Explicit throttle parameters; v0.13-004 §4 |
| Partition healing produces conflicting summary records | v0.7-004 ConflictDomain semantics enforced; v0.13-002 §5 |
| Key rotation under partial connectivity | v0.13-003 §6 explicit handling |

## 7. Out of scope (beyond §4)

- Hardware-rooted recovery (depends on v0.12 secure-element choice;
  software-rooted is the v0.13 baseline).
- Cross-organisational fleets (post-v1.0).
- Predictive fault detection / ML-driven anomaly scoring.
- Recovery from byzantine compromise (multiple cooperating malicious
  nodes); v0.13 assumes a bounded number of compromised nodes per
  the threat model that v0.15 finalises.
