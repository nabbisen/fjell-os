# RFC-v0.7-004: Conflict Domain Metadata and Offline-First Sync Queue

**Status.** Implemented (v0.7.0)

## Status

Draft (revised, supersedes pack v0.7-004 draft)

## Target Version

`v0.7.0`.

## Phase

Distributed Snapshot Sync — Epic D (Conflict / Queue).

## Related Work

- v0.7 RFCs 001/002/003 — identity, snapshot, summary.
- v0.4 RFC 003 — secure-transportd (queue drainer transport).
- v0.8 RFCs 002/003 — fleet aggregation, rollout (consume the queue model).

---

## 1. Summary

Introduce **`ConflictDomain`** metadata that classifies each snapshot
record by *who is authoritative* (the node itself vs. an external source),
and an **offline-first sync queue** that buffers outbound exchange items
when the network is unavailable.

Together they make snapshot/summary exchange survive intermittent
connectivity without sacrificing the local invariants from
RFCs v0.7-002 / 003.

---

## 2. Motivation

Two practical problems surfaced once snapshots and summaries existed:

1. **Conflicts when re-importing a previously-exported snapshot back into
   its origin.** Without a domain tag, the importer cannot tell whether
   incoming records should be ratcheted against (foreign) or merged with
   (own).
2. **Network drops kill operator workflow.** An operator triggers an
   export-push and the network blinks. Today the push fails; the operator
   re-runs manually. A small offline queue handles this transparently.

This RFC adds both as separate, narrow primitives.

---

## 3. Goals

```text
- Each SnapshotRecord and SummarySection carries a ConflictDomain tag.
- Importer rules per domain: Own (refuse), Foreign-Authoritative
  (ratchet/apply), Foreign-Advisory (record only, do not apply).
- Outbound sync queue with bounded capacity, persistence, and ordered drain.
- Queue items are typed by ChannelKind + payload digest.
- Queue is operator-visible and pause-able.
```

## 4. Non-Goals

```text
- No multi-master replication (no CRDTs).
- No background acquisition of remote snapshots (always operator-initiated).
- No retry policy beyond "drain when channel up, fail after N attempts."
- No queue priority beyond FIFO.
```

---

## 5. External Design

### 5.1 ConflictDomain tag

```rust
#[repr(u8)]
pub enum ConflictDomain {
    /// Record describes the local node and cannot be overwritten by import.
    Own              = 0x01,
    /// Record from an external authority; importer ratchets/applies per
    /// kind-specific rules (e.g., RollbackRecord ratchets monotonically).
    ForeignAuthoritative = 0x02,
    /// Record from an external source recorded for visibility only; never
    /// affects local control-plane decisions.
    ForeignAdvisory  = 0x03,
}
```

### 5.2 Per-kind domain table

```text
RecordKind                          Domain
PlatformProfileLoaded (own)         Own
BoardProfileLoaded (own)            Own
MeasurementHeadSummary (own)        Own
MeasurementHeadSummary (peer)       ForeignAdvisory
KeyringSnapshotDigest (own)         Own
KeyringSnapshotDigest (peer)        ForeignAdvisory
RollbackRecord (own)                Own
RollbackRecord (peer)               ForeignAuthoritative   ← ratchet semantics
NodeIdentityRecord (own)            Own
NodeIdentityRecord (peer)           ForeignAuthoritative   ← cached for re-import
AttestationRecordHeadDigest (own)   Own
AttestationRecordHeadDigest (peer)  ForeignAdvisory
ConfigSummaryDigest (own)           Own
ConfigSummaryDigest (peer)          ForeignAdvisory
```

### 5.3 Outbound queue shape

```rust
pub const SYNC_QUEUE_CAP: usize = 16;

pub struct QueueItem {
    pub item_id:        [u8; 8],
    pub kind:           SyncItemKind,        // Snapshot | MeasurementSummary | ReleaseSummary | DiagnosticBundle
    pub payload_digest: Digest32,
    pub size_bytes:     u32,
    pub target:         ChannelKind,          // RFC v0.4-003
    pub server_name:    [u8; 64],
    pub attempts:       u8,
    pub last_attempt_tick: u64,
    pub state:          QueueItemState,
}

#[repr(u8)]
pub enum QueueItemState {
    Pending = 1,
    InFlight = 2,
    Delivered = 3,
    Failed = 4,        // exceeded MAX_ATTEMPTS
    Cancelled = 5,
}
```

Persisted body bytes live alongside the queue item in storaged (record kind
`StoreRecordKind::QueueBody = 0x16`); body retained until Delivered or
Cancelled.

---

## 6. Data Model

### 6.1 Snapshot record extension

RFC v0.7-002's `SnapshotRecord` gains a leading `domain u8` byte:

```text
SnapshotRecord {
    domain u8,            // ConflictDomain tag (NEW)
    kind u16,
    seq u64,
    body_len u32,
    body [u8; body_len],
}
```

The snapshot_digest canonical formula in RFC v0.7-002 is amended:

```text
... for each record:
    domain u8 || kind u16 LE || seq u64 LE || body_len u32 LE || body
```

This is a **`BREAKING-SCHEMA`** change to the snapshot format. The schema
version bumps to 2; v1 readers are not required to accept v2, but v2
readers MUST accept v1 (defaulting absent domain to ForeignAuthoritative
for compat). See §10.

### 6.2 Queue persistence

```rust
pub struct QueueRecord {
    pub schema_version: u16,
    pub items:          [Option<QueueItem>; SYNC_QUEUE_CAP],
    pub record_digest:  Digest32,
}
```

Persisted on every queue mutation; replay scan reconstructs queue at boot.

---

## 7. Internal Design

### 7.1 Importer rules (extension of RFC v0.7-002)

```text
for each record:
  match record.domain:
    Own:
      refuse — never apply (covers AnyOwnRecord protection from v0.7-002).
    ForeignAuthoritative:
      apply per kind-specific rule (rollback ratchet, identity dedup).
    ForeignAdvisory:
      append into storaged with kind tagged "advisory" but never consulted
      by control-plane decisions.
```

### 7.2 Queue drainer service: `syncd`

```text
on push(item):
  if queue.len() == SYNC_QUEUE_CAP and lowest-priority item still Pending:
      reject Err(QueueFull)
  storaged.append QueueBody { item.body }
  storaged.append QueueRecord { item }
  audit: SyncQueueItemEnqueued { item_id, kind }

on drain (periodic when channel up):
  for item in queue.where(state == Pending).order_by(enqueued_tick):
      item.state = InFlight
      result = secure_transportd.push(item.target, item.payload_digest)
      if result.ok:
          item.state = Delivered
          storaged.append SyncDelivery { item_id }
          audit: SyncQueueItemDelivered { item_id }
      else if item.attempts >= MAX_ATTEMPTS:
          item.state = Failed
          audit: SyncQueueItemFailed { item_id, last_error }
      else:
          item.state = Pending
          item.attempts += 1
          item.last_attempt_tick = now
```

### 7.3 Queue control surface

```text
$ fjell-tools sync queue list
$ fjell-tools sync queue pause
$ fjell-tools sync queue resume
$ fjell-tools sync queue cancel <item_id>
```

Pause sets a flag in `configd` that drainer respects between attempts.

### 7.4 Backpressure to producers

If the queue is full, `snapshotd`'s push API returns `QueueFull`. Operators
see this in `fjell-tools snapshot push` output. There is no automatic
eviction; the operator explicitly cancels old items.

---

## 8. Security Design

### 8.1 Threat model deltas

```text
Threat T-190: Re-importing own snapshot used to bypass own-identity refusal.
Mitigation:  domain=Own records refused universally; even cached
             ForeignAuthoritative version of own identity is rejected
             when its node_id matches local node_id.

Threat T-191: Foreign-advisory record consulted as authority.
Mitigation:  storage tag separates advisory from authoritative; readers
             that consult storaged.latest(kind) only see authoritative
             records. Advisory records have a separate access path.

Threat T-192: Queue used as exfiltration vector (operator enqueues a
             payload, captures it elsewhere).
Mitigation:  push operations require ChannelCap; queue items inherit the
             same cap-broker policy as direct push (RFC v0.4-005).

Threat T-193: Queue grows unbounded and exhausts storage.
Mitigation:  SYNC_QUEUE_CAP hard limit; total persisted body bytes capped
             at MAX_QUEUE_BYTES (8 MiB).

Threat T-194: Stale queue replay after long offline — items send
             outdated state.
Mitigation:  each item carries its origin tick; transport server can
             decide what to do (out of scope) but item drop policy lets
             operator cancel stale items.
```

### 8.2 Audit emission

```text
SyncQueueItemEnqueued      { item_id, kind, size }
SyncQueueItemDelivered     { item_id, attempts }
SyncQueueItemFailed        { item_id, last_error }
SyncQueueItemCancelled     { item_id }
SyncQueuePaused            { by_operator }
ConflictDomainViolation    { kind, attempted_domain, expected }
```

---

## 9. Memory / Resource Design

- QueueRecord ≈ 16 × 120 B ≈ 2 KiB.
- Bodies separate, each ≤ 1 MiB (snapshot max from RFC v0.7-002).
- Total queue body cap: 8 MiB.

---

## 10. Compatibility and Migration

### 10.1 Snapshot v2 vs v1

- v2 readers handle v1 by treating absent domain as
  `ForeignAuthoritative` (the v1 default semantics).
- v2 writers always emit v2; commit body carries `BREAKING-SCHEMA: snapshot`.
- Migration ADR enumerates exactly the digest formula change and how the
  frozen-schema file (RFC v0.6-003) is updated.

### 10.2 syncd / storaged

- New record kind 0x16 (QueueBody) and 0x17 (SyncDelivery / pruning).
- Older logs without queue records: queue starts empty.

---

## 11. Test Strategy

### 11.1 Host unit tests

```text
- domain_tag_serialise_round_trip
- snapshot_v2_digest_covers_domain_byte
- snapshot_v2_reader_accepts_v1_default_domain
- own_record_refused_in_import
- foreign_advisory_recorded_not_applied
- queue_enqueue_until_full_returns_error
- queue_persist_then_replay_round_trip
- queue_drain_advances_attempts
- queue_failure_after_max_attempts
- queue_cancel_clears_body
- queue_pause_blocks_drain
```

### 11.2 QEMU smoke

```text
- SMOKE:SYNC:QUEUE_ENQUEUE_DRAIN_HAPPY
- SMOKE:SYNC:QUEUE_SURVIVES_REBOOT
- SMOKE:SYNC:DRAIN_PAUSED_UNTIL_RESUMED
```

### 11.3 Negative

| Marker                                                  | Profile |
|---------------------------------------------------------|---------|
| `NEG:SYNC:QUEUE_FULL_REJECTED`                          | sync    |
| `NEG:SYNC:OWN_DOMAIN_REIMPORT_REJECTED`                 | sync    |
| `NEG:SYNC:ADVISORY_NEVER_AUTHORITATIVE`                 | sync    |
| `NEG:SYNC:CHANNEL_DOWN_RETRIES_THEN_FAILS`              | sync    |
| `NEG:SYNC:CANCELLED_ITEM_NOT_DELIVERED`                 | sync    |

---

## 12. Acceptance Criteria

```text
- syncd ships.
- Snapshot v2 lands with frozen schema update.
- 11 host tests + 3 SMOKE + 5 NEG green.
- Operator CLI commands available.
- BREAKING-SCHEMA: snapshot ADR filed.
- ADR-v0.7-004 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/architecture/v0.7-004-conflict-sync.md
docs/src/format/snapshot-envelope.md        — updated to v2
docs/src/format/sync-queue.md
docs/src/operator/sync-cli.md
docs/src/adr/v0.7-004-conflict-domain.md
docs/src/adr/v0.7-004-snapshot-v2-breaking.md
```

---

## 14. Open Questions

1. **Priority queue** — currently FIFO. If summary pushes need to overtake
   large snapshot pushes, priority can be added. Out of scope for v0.7.
2. **TTL per item** — operator may want "drop if not delivered in 24h."
   Tick-based TTL is straightforward; deferred to v0.8 fleet RFC.
3. **Queue across reboots & secure-transportd state** — queue body is
   persisted; the channel re-establishes on demand. Verified by
   `SMOKE:SYNC:QUEUE_SURVIVES_REBOOT`.

---

## 15. Release Gate (RFC-local)

```text
- syncd ships, snapshot v2 lands.
- 11 host + 3 SMOKE + 5 NEG green.
- BREAKING-SCHEMA + 2 ADRs Accepted.
- CHANGELOG entries filed.
```
