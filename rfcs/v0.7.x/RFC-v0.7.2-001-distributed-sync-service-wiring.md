# RFC-v0.7.2-001: Distributed Sync Service IPC Wiring

## Status

Draft (closes review findings **W-RB-03, C-H-07**)

## Target Version

`v0.7.2`

## Summary

Convert `identityd`, `summaryd`, and `syncd` from self-check stubs into
minimal IPC services that persist and load state through `storaged`,
read measurement and release counters from `measuredd`/`upgraded`, and
expose a capability-gated import skeleton for incoming snapshots.
Also unblock `rootfsd`, `snapshotd`, and `fjell-driver-virtio-blk`
where they overlap with v0.7 sync.

## Motivation

Whole-project review §4 RB-03 and crates review §6 H-07: all three v0.7
services exist as crates but do not perform their job.  The v0.7.0
handoff explicitly named this as deferred to v0.7.x.

The architect ruled (whole-project §8.5) that the format-first /
stub-service pattern is acceptable as a transitional milestone only if
v0.7.x closes the gap before v0.8.  This RFC closes it.

Adjacent service stubs identified in C-H-07 also unblock here because
they live on the same code paths (`rootfsd`, `snapshotd`,
`driver-virtio-blk`).

## Goals

```text
- identityd persists NodeIdentity to storaged at first boot;
  loads it on subsequent boots.
- identityd refuses to ready with a zero identity_digest
  (cooperates with RFC-v0.7.2-003 safe constructor).
- summaryd reads chain head and counters from measuredd / upgraded
  via capability-gated IPC.
- summaryd signs both summaries via attestd and writes them to
  storaged.
- syncd exposes an import endpoint capability-gated to the cap-broker.
- syncd verifies SnapshotEnvelope signatures, identity, and refuses on
  any failure with the typed SnapshotImportError.
- snapshotd produces real snapshots when invoked or is removed from
  the release manifest.
- rootfsd exposes read-only rootfs metadata via service-api or is
  marked nonfunctional in the manifest.
- driver-virtio-blk: declare scope (when block I/O lands) or remove
  from release.
```

## Non-Goals

```text
- No conflict-domain merge implementation (that is RFC-v0.7.2-002).
- No replay cache implementation (that is RFC-v0.7.2-003).
- No outbound sync queue persistence (deferred to v0.7.2.1 patch).
- No multi-peer transport (single peer in v0.7.2; fleet in v0.8).
```

## External Design

### IPC tags introduced

```rust
// crates/fjell-service-api/src/v0_7.rs

pub const IPC_IDENTITY_LOAD:      u16 = 0x0700;
pub const IPC_IDENTITY_PERSIST:   u16 = 0x0701;
pub const IPC_IDENTITY_GET:       u16 = 0x0702;

pub const IPC_SUMMARY_MEASURE:    u16 = 0x0710;
pub const IPC_SUMMARY_RELEASE:    u16 = 0x0711;
pub const IPC_SUMMARY_PERSIST:    u16 = 0x0712;

pub const IPC_SYNC_IMPORT:        u16 = 0x0720;
pub const IPC_SYNC_EXPORT:        u16 = 0x0721;
pub const IPC_SYNC_STATUS:        u16 = 0x0722;
```

These tags require corresponding additions to:

```text
- fjell-cap-broker policy bundle (which caps can invoke which tags)
- fjell-semantic-v1 catalog entries (status semantic intents)
```

### identityd lifecycle

```text
Boot:
  1. Request IPC_STORE_READ for STORE_RECORD_KIND_IDENTITY (0x0020).
  2. If present, deserialize, recompute identity_digest, compare.
       - mismatch → audit IDENTITY_DIGEST_MISMATCH and refuse to ready.
  3. If absent:
       - generate node_id (random via trust-provider RNG).
       - construct NodeIdentity with new_with_digest() (RFC v0.7.2-003).
       - persist via IPC_STORE_APPEND with kind 0x0020.
  4. Publish IPC_IDENTITY_GET endpoint.
  5. Emit "identityd: ready node_id=<hex>".
```

### summaryd lifecycle

```text
Periodic (every 60 s, configurable):
  1. IPC_MEASURE_HEAD_GET (measuredd) → head_seq, head_chain_digest, kind counts.
  2. IPC_RELEASE_COUNTERS_GET (upgraded) → channel summaries.
  3. Construct MeasurementSummary, ReleaseSummary, finalize digests.
  4. IPC_ATTEST_SIGN_SUMMARY (attestd) → signed envelopes.
  5. IPC_STORE_APPEND (storaged) with kinds 0x0030, 0x0031.
  6. Publish summary digests via semantic-stream intents
     0x0180 (SUMMARY.MEASUREMENT_EMITTED) and
     0x0181 (SUMMARY.RELEASE_EMITTED).
```

### syncd import skeleton

```text
On IPC_SYNC_IMPORT:
  1. Caller must hold SyncImport capability.
  2. Read SnapshotEnvelope from caller-provided buffer.
  3. Validate schema_version is supported (v1 or v2).
  4. Compute snapshot_digest, compare with declared digest.
  5. Look up source identity via IPC_IDENTITY_GET (peer node id).
  6. Validate signature via attestd verify-snapshot-signature path.
  7. Check NodeIdentityPolicy.permits(peer.trust_profile_tag).
       - Fleet mode requires roster validation (RFC v0.7.2-003).
  8. Reject on any failure with the typed SnapshotImportError.
  9. On Accepted: queue records for merge (merge rules in RFC-v0.7.2-002).

v0.7.2 returns SnapshotImportOutcome::PartialDryRun for any positive
case, until RFC-v0.7.2-002 lands merge rules. This makes
"Accepted" unreachable until both RFCs are merged.
```

## Data Model

### Persisted identity record

```rust
pub struct PersistedIdentity {
    pub schema_version: u16,
    pub node_identity:  NodeIdentity,  // 192 B
}
// Stored at storaged STORE_RECORD_KIND_IDENTITY (0x0020).
```

### Summary records

```rust
pub struct PersistedMeasurementSummary {
    pub schema_version:  u16,
    pub signed_envelope: SignedSummary<MeasurementSummary>,
}

pub struct PersistedReleaseSummary {
    pub schema_version:  u16,
    pub signed_envelope: SignedSummary<ReleaseSummary>,
}
// Kinds 0x0030 (MeasurementSummary) and 0x0031 (ReleaseSummary).
```

`SignedSummary<T>` wraps the canonical type with the attestd signature
descriptor (see `fjell-attestation-format::SignedByDescriptor`).

## Internal Design

### `fjell-service-api` extensions

A new `v0_7` module containing the IPC tag constants and the request/
response structs.  Backward-compatible additive change.

### Capability bindings

cap-broker policy bundle extension:

```text
- identityd holds: StoreRead/StoreAppend for kind 0x0020
- summaryd holds: StoreAppend (0x0030, 0x0031),
                  MeasureRead,
                  ReleaseCountersRead,
                  AttestSignSummary
- syncd holds:    StoreAppend (0x0040 import log),
                  AttestVerifyAny,
                  IdentityGet,
                  ServiceApiSyncImport (the exposed endpoint cap)
```

### Error contract

Every IPC reply carries either the typed response or a
`ServiceError`:

```rust
pub enum ServiceError {
    NotPermitted,
    InvalidArgument,
    ResourceExhausted,
    StorageFailure,
    AttestationFailure,
    SchemaTooNew,
    InternalError,
}
```

For sync import, the more specific `SnapshotImportError` is used.

## Security Design

### What an attacker controls in v0.7.2

- The bytes of an inbound `SnapshotEnvelope` (via the `IPC_SYNC_IMPORT`
  endpoint).  This requires the caller to already hold `SyncImport`,
  which is cap-broker policy-gated.

### Defences

- Signature verification before any state mutation.
- Identity policy check before signature trust.
- Schema version is rejected if unknown.
- Buffer overflow defence (RFC-v0.7.2-002 §C-RB-02).
- Until merge rules land, all positive verifications result in
  `PartialDryRun`, never `Accepted`.  This makes the failure mode
  fail-closed: the worst outcome is a snapshot that verifies but is
  not applied.

### Threat: stale identity persistence

If an attacker corrupts the persisted identity record:

- identityd recomputes the digest on load and compares.  Mismatch
  audits `IDENTITY_DIGEST_MISMATCH` and refuses to ready.
- The kernel does not spawn dependent services if identityd is not
  ready.  System enters recovery.

## Memory / Resource Design

- identityd: 192-byte record, single read/write per boot.
- summaryd: ~512 B per summary, persisted on a 60 s timer.
- syncd: incoming buffer bounded by `MAX_SNAPSHOT_ENVELOPE_BYTES`
  (declared in RFC-v0.7.2-002).

## Compatibility and Migration

- First boot after v0.7.2 upgrade: identityd generates a fresh
  `NodeIdentity` and persists it.  This is a one-time event.
- Existing storaged records are untouched.  New kinds (0x0020, 0x0030,
  0x0031, 0x0040) are additive.

## Test Strategy

```text
- Unit tests for each new IPC tag (request/response round-trip).
- Integration test: identityd persists, restarts, recomputes digest,
  matches.
- Integration test: identityd refuses to ready on digest mismatch.
- Integration test: summaryd produces non-zero summary digests on a
  populated measurement chain.
- Integration test: syncd rejects a tampered envelope (signature
  failure).
- Integration test: syncd rejects an envelope from an unlisted profile
  (TrustMode::SameFamily policy).
- QEMU smoke v0.7-sync produces TEST:V0.7-SYNC:PASS after these
  services genuinely persist and respond to IPC.
```

## Acceptance Criteria

```text
- SnapshotImportOutcome::Accepted is unreachable in v0.7.2 (returns
  PartialDryRun on success path).
- identityd, summaryd, syncd remain alive after their initial setup
  (do not exit).
- TEST:V0.7-SYNC:PASS includes identityd persistence round-trip.
- Tampered envelope rejection logged as
  AUDIT_SYNC_IMPORT_SIG_FAIL.
- ADR-v0.7.2-001 filed.
```

## Documentation Requirements

```text
- docs/src/reference/service-api-v0_7.md added; lists every new IPC tag.
- docs/src/internals/identityd-lifecycle.md added.
- docs/src/internals/summaryd-lifecycle.md added.
- docs/src/internals/syncd-import-pipeline.md added.
```

## Open Questions

```text
1. Should summaryd push or pull? RFC chose push (summaryd periodically
   collects). Alternative: pull (consumer asks summaryd, summaryd
   gathers on demand). Decision: stay with push, simpler accounting.

2. What happens if summaryd loses an IPC reply from attestd? Proposal:
   timeout + retry up to 3 times; then audit
   AUDIT_SUMMARY_SIGN_TIMEOUT and back off 60 s.

3. syncd's "incoming buffer" is in kernel-owned memory? Proposal: no;
   syncd allocates from its task heap. The kernel only routes the IPC.
```

## Release Gate

`TEST:V0.7-SYNC:PASS` from RFC-v0.7.1-003 is upgraded to require:

```text
- "identityd: persisted node_id=<hex>"
- "identityd: reloaded node_id=<hex> digest_ok"
- "summaryd: emitted measurement_summary head_seq=N"
- "summaryd: emitted release_summary channels=N"
- "syncd: import endpoint ready"
```

A v0.7.2 release is gated on this expanded smoke marker set.
