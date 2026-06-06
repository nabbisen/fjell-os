# RFC-v0.13-002 — Fleet Split, Reconnect, and Reconciliation

**Status:** Implemented (v0.13.0)
**Target version:** v0.13.0
**Parent:** v0.13-001.
**Cross-refs:** RFC v0.7-004 (ConflictDomain), v0.8-002 (semantic aggregation).

## 1. Problem

The v0.7 sync queue and v0.7-004 conflict-domain metadata describe how a
single node defers and reconciles its sync state. They do not describe
how a *fleet* — a coordinator and N members — behaves when network
partition splits the fleet, allows independent evolution on either
side, and then heals.

Without partition semantics:

- Re-merged state can double-count measurements.
- A node that missed a rollout while partitioned can boot a stale
  bundle.
- Authority decisions taken on one side of the partition may conflict
  with those on the other.

v0.13 lands a partition-aware state machine with explicit reconciliation
rules.

## 2. Partition model

A *partition* is any subset of fleet nodes that cannot reach the
coordinator for longer than the configured `partition_threshold`
(default: 5 × heartbeat interval). The fleet at any moment is in one
of four states:

```text
        Healthy
           │ heartbeat lost
           ▼
        Suspect ── reconnect ──► Healthy
           │ partition_threshold reached
           ▼
        Partitioned
           │ link restored
           ▼
        Reconciling ──► Healthy   (after RFC §4 procedure)
```

Each node also tracks its *side* — `WithCoordinator` or
`PartitionedAway`. The coordinator always considers itself
`WithCoordinator`; nodes do so until they fail to deliver heartbeats.

## 3. Independent-side behaviour

While partitioned, a node:

- Continues to accept signed bundles already staged before the partition.
- **Refuses** to act on a new rollout intent unless that intent
  carries a coordinator signature dated before the partition began.
- Continues to emit measurements and audit records into its local sync
  queue.
- Does **not** advance any KeyEpoch (RFC-v0.11-004).
- Does **not** accept a roster modification.

A `PartitionedAway` node enters a fail-safe authority posture: its
authority surface shrinks to what was permitted before the partition.
The coordinator, on the `WithCoordinator` side, may continue
operating but cannot make decisions that bind the partitioned nodes
retroactively — see §4.

## 4. Reconciliation procedure

When link is restored:

1. Both sides exchange their `ConflictDomain` summaries plus a
   monotonic per-node `sync_seq`.
2. The coordinator computes the merge plan: every record produced on
   the partitioned side is examined.
3. For each record:
   - **Measurement / audit / semantic evidence:** merge unconditionally
     (the records are content-addressed; duplicates are deduped by
     digest).
   - **Roster / policy / rollout state changes:** the coordinator's
     pre-partition decision wins; partitioned-side updates of these
     classes are rejected with a `RECONCILE.REJECT_AUTHORITY` audit
     event.
   - **KeyEpoch changes proposed on the partitioned side:** rejected.
4. The merge plan itself becomes a signed `ReconcileManifest`
   committed by the coordinator; nodes apply only after verifying
   the signature.
5. After all nodes acknowledge `ReconcileManifest`, the fleet returns
   to `Healthy`.

## 5. Evidence preservation

The reconciliation must not lose evidence:

- Records refused as authority-conflicting are *not* discarded; they
  are kept in the audit ring as `RECONCILE.REJECTED`, with the reason
  code and the conflicting record's digest.
- Operators can replay the partitioned-side audit later for forensics;
  the fleet state machine refuses to treat them as authoritative but
  the bytes are preserved.

## 6. Coordinator failover (limited)

If the coordinator itself is the partitioned-away side, the surviving
member-only side must:

- Refuse to elect a new coordinator. v0.13 does not implement leader
  election — a deliberate restriction.
- Continue in `Partitioned, no coordinator` posture indefinitely.
- Refuse all coordinator-side operations.
- Wait for operator intervention via the v0.13-005 disaster runbook,
  which describes how to promote a surviving node when the original
  coordinator is truly gone.

Leader election is deferred to v1.x research; the v1.0 invariant is
that a fleet without a reachable coordinator is *visibly* without one
and refuses to fabricate authority.

## 7. New catalog intents

Allocated under the reserved fleet range:

- `FLEET.PARTITION_DETECTED` — heartbeat threshold exceeded.
- `FLEET.RECONCILE_STARTED` — coordinator initiated reconciliation.
- `FLEET.RECONCILE_REJECTED` — record refused as authority-conflicting.
- `FLEET.RECONCILE_COMPLETED` — fleet returned to Healthy.

## 8. Acceptance criteria

1. Each node tracks fleet-state {Healthy, Suspect, Partitioned,
   Reconciling}; transitions emit the corresponding catalog intent.
2. Member nodes in `PartitionedAway` refuse roster/policy/rollout
   updates not pre-signed before partition.
3. `ReconcileManifest` is defined, produced by the coordinator,
   verified by members.
4. Reference fleet (RFC-v0.10-005) gains a partition scenario:
   isolate node-C for 60 s, change roster on node-A, restore link;
   assert node-C refuses the post-partition roster update and a
   `ReconcileManifest` reconciles the state correctly.
5. Evidence from the partitioned side is preserved in the audit
   record.
6. Coordinator-side partition (coordinator is the partitioned side)
   produces `Partitioned, no coordinator` posture on the survivors,
   no leader election.

## 9. Out of scope

- Leader election / quorum.
- Multi-coordinator topologies.
- Conflict resolution for measurement *content* (the records are
  immutable; "conflict" here is authority-class only).
- Bandwidth-aware reconciliation pacing (deferred to v1.x).
