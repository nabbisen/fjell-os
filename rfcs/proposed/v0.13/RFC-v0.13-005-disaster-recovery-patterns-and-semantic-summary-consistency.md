# RFC-v0.13-005 — Disaster Recovery Patterns and Semantic Summary Consistency

**Status:** Proposed
**Target version:** v0.13.0
**Parent:** v0.13-001.
**Cross-refs:** RFC v0.8-004 (recovery intent), v0.7-003 (release
    summary sync), v0.13-002, v0.13-003.

## 1. Problem

The previous three v0.13 RFCs each handle one failure class —
partition, key compromise, rollout failure. Real incidents are
multi-class: the network is bad *and* the coordinator is down *and* a
key may be compromised. v0.13-005 ships:

- A consolidated disaster-recovery runbook covering composite failures.
- Mechanical consistency checks on semantic summaries (RFC v0.7-003)
  so the operator can trust the recovery decisions they make.

## 2. Composite scenarios

The runbook catalogues these:

| ID | Scenario | Primary RFC | Composite element |
|----|----------|-------------|-------------------|
| DR1 | Coordinator hardware loss | v0.13-002 | Survivors form `Partitioned, no coordinator` |
| DR2 | Coordinator hardware loss + key compromise | v0.13-002+003 | Survivors hold no rotation authority |
| DR3 | Full fleet power loss + on-restart key compromise discovered | v0.13-003 | Re-attestation drives quarantine decisions |
| DR4 | Rollout failure during partition heal | v0.13-002+004 | Reconcile manifest carries rollback |
| DR5 | Bulk corruption of audit storage on a single node | local | Node is quarantined, re-enrolled |
| DR6 | Trust Anchor Root key lost | v0.13-003 §3 S4 | Operator break-glass procedure |
| DR7 | Schema migration aborted mid-fleet | v0.10-002 | Mixed-catalog fleet handling |
| DR8 | Operator mistake: revoked the wrong key | v0.13-003 | Manual recovery via TrustAnchorRoot |

Each entry has a runbook section with:

- Symptoms (what the Trust Report and audit show).
- Containment (immediate actions).
- Diagnosis (questions to answer before recovery).
- Recovery (concrete steps; references the v0.13-002/003/004 commands).
- Verification (criteria for "back to normal").
- Post-incident attestation (what to commit to the audit chain).

The runbook is committed at `docs/operations/disaster-recovery.md`.

## 3. Semantic summary consistency

The summary records (RFC v0.7-003 `MeasurementSummary`,
`ReleaseSummary`) describe a node's state at a point in time. Without
consistency rules they can drift:

- A summary may reference catalogue tags that have been retired in a
  later catalog version.
- A summary may name a bundle by `bundle_digest` that has been
  revoked.
- Two summaries from the same node at adjacent timestamps may show
  causally impossible transitions.

v0.13 adds a consistency checker run by the coordinator on every
ingested summary:

### 3.1 Static checks

- Every referenced catalog tag exists in the catalog version the
  summary claims to target.
- Every referenced `bundle_digest` either exists in the coordinator's
  bundle ledger or is flagged `UnknownBundle`.
- Required fields per `MeasurementSummary` shape are present and
  type-correct.

### 3.2 Temporal checks

For each node, the coordinator maintains a small "last-known-good"
state. On a new summary:

- `sync_seq` strictly greater than the previous.
- Lifecycle transitions in the summary respect the bundle lifecycle
  FSM (Fetched→Verified→Committed→Running→{Confirmed,RolledBack}).
- `KeyEpoch` does not decrease.
- `boot_count` strictly greater unless the summary is itself a boot
  record.

A failed check produces a `CONSISTENCY.SUMMARY_REJECTED` audit record
with the reason code. The summary is preserved as evidence but not
treated as authoritative.

## 4. Coordinator promotion (limited)

DR1 / DR2 require promoting a surviving node to coordinator. v0.13-005
specifies the only supported procedure:

- The operator runs `cargo xtask fleet promote --node <id>` on the
  candidate node from the operator workstation.
- The operation requires the `TrustAnchorRoot` key (RFC-v0.13-003 §5).
- The result is a signed `CoordinatorPromotion` record distributed to
  the surviving members.
- Members verify the signature, update their coordinator binding, and
  resume normal heartbeating.

The procedure is **operator-driven**, signed by the highest-authority
key in the system, and auditable. No automatic leader election. This
is consistent with RFC 061's identity statement: every authority is
explainable, and the most consequential authority — choosing who
speaks for the fleet — is the most explainable of all.

## 5. Evidence retention and replay

After any DR scenario closes, the operator commits:

- The audit ring segment covering the incident.
- The signed manifests produced (rotation, revocation, reconcile,
  promotion).
- The runbook section followed, with any deviations noted.
- A short narrative committed to `docs/operations/incidents/<date>.md`.

These artefacts feed the next Trust Report and the v0.15 threat-model
finalisation: incident data tells us which threats were exercised.

## 6. Drills

`tests/qemu/profiles/dr-drill.toml` exercises the CI-feasible
scenarios:

- DR1 (coordinator loss + survivor refusal to elect).
- DR4 (rollout failure during partition heal).
- DR5 (audit corruption + re-enrolment).
- DR7 (mixed-catalog fleet).
- Promotion procedure (success path).

DR2, DR3, DR6, DR8 require manual operator walkthrough at landing and
are attested in `docs/operations/dr-attestation-v0.13.md`.

## 7. Acceptance criteria

1. `docs/operations/disaster-recovery.md` exists with full coverage
   of DR1–DR8.
2. `tools/fjell-summary-check/` exists, runs the static and temporal
   checks, and is invoked by the coordinator on every ingested
   summary.
3. `cargo xtask fleet promote` exists, requires the `TrustAnchorRoot`
   signature, produces a `CoordinatorPromotion` record, and is
   exercised in CI.
4. The CI DR drill profile passes for DR1, DR4, DR5, DR7 plus
   promotion.
5. DR2, DR3, DR6, DR8 walkthroughs are attested at landing.
6. The Trust Report's "DR posture" subsection lists each DR scenario
   with the timestamp of the last drill or incident.
7. `CONSISTENCY.SUMMARY_REJECTED` emits with reason codes; round-trips.

## 8. Out of scope

- Automated coordinator failover. By design v0.13 requires operator
  promotion.
- Cross-fleet disaster recovery (multi-fleet is post-v1.0).
- Forensic-grade tamper-evident sealed evidence (would require
  hardware roots not yet shipped in v0.12).
- Insurance / SLA implications. The runbook concerns technical
  recovery only.
- Real-time DR for hard-real-time workloads. Fjell does not target RT.
