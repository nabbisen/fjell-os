# RFC-v0.8-002: Semantic State Aggregation and Fleet View

**Status.** Implemented (v0.8.0)

## Status

Draft (revised, supersedes pack v0.8-002 draft)

## Target Version

`v0.8.0`.

## Phase

Fleet Operations Plane — Epic B (State Aggregation).

## Related Work

- v0.7 RFC 003 — MeasurementSummary / ReleaseSummary.
- v0.4 RFC 005 — DiagnosticBundle.
- v0.8 RFC 001 — FleetRoster.
- v0.5 RFC 004 — Semantic catalog v1.

---

## 1. Summary

Define **FleetView** — a fleet-side aggregate of per-node summaries
(`MeasurementSummary`, `ReleaseSummary`, `DiagnosticBundle`) keyed by
fleet_member_id and roster_epoch. Define a **`fleet-view`** service that
runs at the fleet authority host, ingests summaries pushed by enrolled
nodes, and exposes:

- a per-node detail view;
- a fleet-rollup view: "how many nodes at counter X", "how many in
  Quarantined", "how many failed attestation in last hour";
- a signed `FleetViewSnapshot` for export.

This is the first **fleet-side** Fjell component. Nodes do not need to
change; existing push protocols (v0.7 RFC 003, v0.4 RFC 005) carry the
data.

---

## 2. Motivation

A fleet without aggregation is just a set of independent devices. The
view is the operator's window into the fleet's *current shape*: how many
nodes are on which release counter, which are in Quarantined, which have
the latest measurement chain head.

Aggregation must be:

- **derived only from signed summaries** — never trust unsigned device
  reports;
- **bounded** — capped per-node history and per-fleet rollup size;
- **deterministic** — same inputs produce the same view bytes.

---

## 3. Goals

```text
- A view service running on a fleet authority host (Fjell or non-Fjell;
  see §14.1).
- Ingest signed summaries via secure-transportd Diagnostics channel.
- Per-node history bounded to last K summaries (K=8 default).
- Rollup view with stable schema (RFC v0.5-004 catalog applies).
- Export of FleetViewSnapshot signed by FleetViewSigning key.
- Operator CLI for filtered queries.
```

## 4. Non-Goals

```text
- No alerting / paging. View is reportable, not actionable.
- No time-series storage. Bounded history per node, not per-time.
- No automatic remediation. Action lives in v0.8 RFC 004.
- No real-time push to operators. Operator queries pull-style.
```

---

## 5. External Design

### 5.1 Topology

```text
            ┌─────────────────────────────────────────────────┐
            │ fleet authority host                            │
            │   ┌───────────────┐    ┌──────────────────┐     │
            │   │ secure-       │    │ fleet-view       │     │
            │   │ transportd    │    │ (ingest +        │     │
            │   │ (server-side) ├───►│  aggregate)      │     │
            │   └───────────────┘    └──────────┬───────┘     │
            └───────────────────────────────────┼─────────────┘
                                                │
                                                ▼
                                       operator CLI / signed export
```

The server side of `secure-transportd` (which is symmetrical to the
client side from RFC v0.4-003) accepts pushes over the Diagnostics and
Attestation channels and routes signed bodies to `fleet-view`.

### 5.2 Operator workflow

```text
$ fjell-fleet-tool view list
$ fjell-fleet-tool view node <member_id>
$ fjell-fleet-tool view release-rollup
$ fjell-fleet-tool view export --signed --out fleet-view-2026Q2.bin
```

---

## 6. Data Model

### 6.1 Per-node row

```rust
pub const NODE_HISTORY_DEPTH: usize = 8;

pub struct NodeRow {
    pub fleet_member_id:     [u8; 8],
    pub node_id:             NodeId,
    pub member_status:       MembershipStatus,
    pub role:                FleetRole,
    pub last_seen_tick:      u64,                  // authority-side tick
    pub last_measurement:    MeasurementSnap,
    pub last_release:        ReleaseSnap,
    pub last_diag:           DiagSnap,
    pub history_count:       u8,
    pub history:             [HistoryEntry; NODE_HISTORY_DEPTH],
    pub row_digest:          Digest32,
}

pub struct MeasurementSnap {
    pub head_seq:            u64,
    pub head_chain_digest:   Digest32,
    pub policy_digest:       Digest32,
    pub received_tick:       u64,
}

pub struct ReleaseSnap {
    pub primary_channel_id:  [u8; 8],
    pub current_counter:     u64,
    pub min_counter:         u64,
    pub active_anchor_epoch: u32,
    pub received_tick:       u64,
}

pub struct DiagSnap {
    pub bundle_id:           [u8; 8],
    pub bundle_digest:       Digest32,
    pub audit_count:         u8,
    pub critical_intent_count: u8,
    pub received_tick:       u64,
}

pub struct HistoryEntry {
    pub kind:                HistoryKind,  // Measurement | Release | Diagnostic
    pub digest:              Digest32,
    pub at_tick:             u64,
}
```

### 6.2 Rollup view

```rust
pub const MAX_ROLLUP_BUCKETS: usize = 32;

pub struct ReleaseRollup {
    pub channel_id:       [u8; 8],
    pub bucket_count:     u8,
    pub buckets:          [CounterBucket; MAX_ROLLUP_BUCKETS],
}

pub struct CounterBucket {
    pub counter:      u64,
    pub node_count:   u16,
}

pub struct StatusRollup {
    pub pending:       u16,
    pub active:        u16,
    pub quarantined:   u16,
    pub revoked:       u16,
}

pub struct MeasurementRollup {
    pub unique_head_digests: u8,
    pub heads:               [(Digest32, u16); 8],   // top 8 by node count
}
```

### 6.3 Signed snapshot

```rust
pub struct FleetViewSnapshot {
    pub schema_version:   u16,
    pub fleet_id:         [u8; 16],
    pub roster_epoch:     u32,
    pub built_tick:       u64,
    pub node_count:       u16,
    pub release_rollup:   ReleaseRollup,
    pub status_rollup:    StatusRollup,
    pub measurement_rollup: MeasurementRollup,
    pub snapshot_digest:  Digest32,
}

pub struct SignedFleetViewSnapshot {
    pub snapshot:  FleetViewSnapshot,
    pub signature: Signature,
}
```

Signed with `KeyPurpose::FleetRoot` (or a delegated `FleetViewSigning` if
introduced in a v0.8.x ADR). Domain separator
`"FJELL-FLEETVIEW-SIGN-V1"`.

---

## 7. Internal Design

### 7.1 Ingest pipeline

```text
on receive SignedSummary from member M:
  verify member M is Active per current roster
  verify signature using member's attestation_pubkey
  verify summary's source_node_id matches M's node_id
  determine kind (Measurement / Release / Diagnostic)
  update NodeRow.last_<kind> and push into history (FIFO, bounded)
  emit fleet-view audit FleetViewIngested { member_id, kind, digest }
```

### 7.2 Rollup recomputation

Rollups are recomputed on demand (CLI query) or on a scheduled basis
(default every 60 s). Recomputation is O(nodes); fleets up to ~1k nodes
finish in ~ms.

### 7.3 Persistence

NodeRow is persisted authority-side (Fjell-host storaged or external KV,
see §14.1). FleetViewSnapshot is computed in-memory and signed on export.

---

## 8. Security Design

### 8.1 Threat model deltas

```text
Threat T-210: Adversary pushes summaries claiming to be node X.
Mitigation:  signature verified against X's attestation_pubkey from
             roster; source_node_id in the summary must match.

Threat T-211: Node spams summaries to inflate history.
Mitigation:  per-(member, kind) rate-limit on ingest; older entries
             evicted FIFO so spam cannot displace real data without
             matching cadence.

Threat T-212: Summary from a Revoked member is accepted.
Mitigation:  roster status check at ingest; Revoked → drop with audit.

Threat T-213: View snapshot leaks alias/identifier of nodes the operator
             intended to keep pseudonymous.
Mitigation:  alias and node_id are operator-visible by design; if
             pseudonymity is desired, alias is set to a non-identifying
             string at enrollment time.

Threat T-214: Forged FleetViewSnapshot circulates.
Mitigation:  signature over snapshot_digest with FleetRoot purpose;
             consumers verify before treating snapshot as authoritative.
```

### 8.2 Audit emission

```text
FleetViewIngested            { member_id, kind, digest }
FleetViewIngestRefused       { member_id, reason_code }
FleetViewSnapshotBuilt       { snapshot_digest, node_count, roster_epoch }
FleetViewRollupRecomputed    { which_rollup, duration_us }
```

---

## 9. Memory / Resource Design

- NodeRow ≈ 800 B; 1000-node fleet ≈ 800 KiB resident.
- History per node bounded to NODE_HISTORY_DEPTH × 48 B = 384 B.
- Rollups recomputed not persisted.

---

## 10. Compatibility and Migration

- Authority-side component; nodes unchanged.
- secure-transportd server side requires implementation if not already
  present; v0.8 ships the minimal server.

---

## 11. Test Strategy

### 11.1 Host unit tests

```text
- noderow_digest_covers_last_summaries
- history_fifo_eviction
- rollup_counter_buckets_aggregate
- rollup_status_counts
- rollup_measurement_top_n
- snapshot_digest_covers_rollups
- signed_snapshot_round_trip
- ingest_revoked_member_dropped
- ingest_unknown_member_dropped
- ingest_signature_failed_dropped
- ingest_node_id_mismatch_dropped
```

### 11.2 QEMU + authority host smoke

```text
- SMOKE:VIEW:THREE_NODE_INGEST       — 3 QEMU nodes push, view reflects
- SMOKE:VIEW:RELEASE_ROLLUP_BUCKETS  — staggered counters, rollup checks
- SMOKE:VIEW:SNAPSHOT_EXPORT_VERIFY
```

### 11.3 Negative

| Marker                                                  | Profile    |
|---------------------------------------------------------|------------|
| `NEG:VIEW:UNKNOWN_MEMBER_DROPPED`                       | fleet-view |
| `NEG:VIEW:SIGNATURE_FAILED_DROPPED`                     | fleet-view |
| `NEG:VIEW:REVOKED_MEMBER_DROPPED`                       | fleet-view |
| `NEG:VIEW:NODE_ID_MISMATCH_DROPPED`                     | fleet-view |
| `NEG:VIEW:SNAPSHOT_BAD_SIGNATURE_REJECTED`              | fleet-view |
| `NEG:VIEW:RATE_LIMIT_PER_MEMBER`                        | fleet-view |

---

## 12. Acceptance Criteria

```text
- fleet-view binary lands.
- secure-transportd server-side ingest path lands.
- fjell-fleet-view-format crate with all types.
- 11 host tests + 3 SMOKE + 6 NEG markers green.
- Multi-node QEMU CI matrix used for SMOKE.
- ADR-v0.8-002 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/architecture/v0.8-002-fleet-view.md
docs/src/format/fleet-view-snapshot.md
docs/src/operator/fleet-view-cli.md
docs/src/adr/v0.8-002-aggregate-from-signed-only.md
```

---

## 14. Open Questions

1. **Authority-side OS** — must the fleet authority run Fjell? No;
   `fleet-view` and the authority CLI are designed to compile on
   host-tier Linux/macOS as well. Fjell-on-Fjell hosting is a v0.9 dev
   pattern.
2. **Long-term archive** — current design keeps bounded history per
   node. Long-term storage requires either an external sink or a future
   v0.8.x time-series RFC.
3. **PII concerns** — alias and counters are intentionally visible.
   Operator-set aliases that themselves contain PII are a deployment
   concern, not a Fjell concern.

---

## 15. Release Gate (RFC-local)

```text
- fleet-view ships; CI matrix runs multi-node QEMU.
- 11 host + 3 SMOKE + 6 NEG markers green.
- ADR Accepted.
```
