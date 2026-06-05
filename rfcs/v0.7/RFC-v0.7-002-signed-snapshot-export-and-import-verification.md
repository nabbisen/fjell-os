# RFC-v0.7-002: Signed Snapshot Export and Import Verification

## Status

Draft (revised, supersedes pack v0.7-002 draft)

## Target Version

`v0.7.0`.

## Phase

Distributed Snapshot Sync — Epic B (Export / Import).

## Related Work

- v0.7 RFC 001 — NodeIdentity (the signer of an export).
- v0.3 RFC 002 — keyring (`KeyPurpose::SnapshotSigning`).
- v0.4 RFC 003 — secure-transportd (the transport for exchange).
- v0.7 RFCs 003 / 004 — consume exported snapshots.

---

## 1. Summary

Define **`SignedSnapshot`** — a versioned, signed export of a Fjell node's
durable state subset. Define the export pipeline and the importer's strict
verification pipeline. Constrain *what* can be exported (a curated subset
of records, not arbitrary storage bytes).

This RFC delivers operator-initiated export/import. Background or
fleet-wide propagation is later (RFC v0.7-003 for summary sync, v0.8-002
for fleet aggregation).

---

## 2. Motivation

Fjell needs node-to-node migration:

- moving a device's configuration / measurement history to a successor;
- recovering from board failure (state survives, identity changes);
- pre-staging a fleet from a reference node.

A "snapshot" needs strict shape so it cannot smuggle policy:

- only enumerated record kinds;
- signed end-to-end;
- replay-safe import (idempotent or refused).

---

## 3. Goals

```text
- Strict snapshot shape with versioned schema.
- Exported records limited to a curated allow-list.
- Snapshot signed by NodeIdentity attestation key (KeyPurpose:
  AttestationSigning, with a SNAPSHOT domain separator).
- Importer verifies signature + freshness + identity policy (RFC v0.7-001).
- Import is *atomic*: either every accepted record lands or none does.
- Import preserves anti-rollback (RFC v0.3-003) — never lowers any
  min_counter.
```

## 4. Non-Goals

```text
- No general-purpose backup. Snapshots carry *state*, not blobs.
- No support for diff / delta in v0.7.0; full snapshots only.
- No automatic conflict resolution beyond "newer wins per record kind"
  + anti-rollback ratchet.
- No secret material in snapshots; sealed keys remain provider-bound and
  are *never* exported.
```

---

## 5. External Design

### 5.1 Operator workflow

```text
node A (source):
   $ fjell-tools snapshot export --out alice.snap

node B (target):
   $ fjell-tools snapshot import --in alice.snap --from <node-A id>
```

The import command requires:

- a `SignedNodeIdentity` for A;
- a `Snapshot.snap` file;
- the importer's local NodeIdentityPolicy (v0.7 RFC 001) to permit A.

### 5.2 Snapshot composition

```text
SnapshotEnvelope {
    schema_version u16 = 1,
    source_identity_digest 32 B,        // identity_digest of NodeA
    issued_tick u64,
    nonce 16 B,                         // operator-or-target challenge
    record_count u16,
    records [SnapshotRecord; record_count],
    snapshot_digest 32 B,
    signature 64 B,
}

SnapshotRecord {
    kind u16,                           // restricted to curated set, see §6.2
    seq  u64,                           // source seq within that kind
    body_len u32,
    body [u8; body_len],
}
```

### 5.3 Allow-listed record kinds

```text
0x10  PlatformProfileLoaded (digest only, no body)
0x11  BoardProfileLoaded    (digest only)
0x12  MeasurementHeadSummary (last K entries, see RFC v0.7-003)
0x13  KeyringSnapshotDigest (digest only, anchors not exported)
0x14  RollbackRecord        (per-channel)
0x15  NodeIdentityRecord    (own)
0x16  AttestationRecordHeadDigest (the latest v2 record_digest, no signature)
0x17  ConfigSummaryDigest   (operator-set config digests)
```

Records outside this set are stripped at export. Importers reject unknown
kinds with `SnapshotError::UnknownRecordKind`.

---

## 6. Data Model

### 6.1 Canonical snapshot digest

```text
snapshot_digest = SHA256(
    "FJELL-SNAPSHOT-V1" ||
    schema u16 LE ||
    source_identity_digest 32 B ||
    issued_tick u64 LE ||
    nonce 16 B ||
    record_count u16 LE ||
    for each record:
        kind u16 LE || seq u64 LE || body_len u32 LE || body[body_len]
)
```

`signature = trust_provider.sign_attestation(SnapshotDigestDomain(snapshot_digest))`
where `SnapshotDigestDomain` is a typed wrapper that prepends
`"FJELL-SNAPSHOT-SIGN-V1"` before computing the inner digest passed to
the provider — preventing cross-protocol replay against attestation
records.

### 6.2 Import outcome

```rust
pub enum SnapshotImportOutcome {
    Accepted { records_applied: u16, records_skipped: u16 },
    Refused  { reason: SnapshotImportError },
    PartialDryRun { would_apply: u16 },     // --dry-run flag
}

pub enum SnapshotImportError {
    SignatureFailed,
    IdentityNotPermitted,
    DigestMismatch,
    NonceFailed,
    UnknownRecordKind,
    RollbackViolation { channel_id: [u8; 8], peer_counter: u64, local_counter: u64 },
    Replay { snapshot_id: [u8; 16] },
    TooLarge,
}
```

### 6.3 Import nonce protocol

For pairwise exchange over secure-transportd:

```text
1. target → source: open ChannelKind::FleetEnroll (RFC v0.4-003)
2. target → source: send 16 B challenge nonce
3. source builds snapshot with nonce in envelope
4. source → target: SignedSnapshot
5. target verifies + applies
```

For offline exchange (file), the snapshot's nonce is operator-provided.

---

## 7. Internal Design

### 7.1 Export pipeline (`snapshotd export`)

```text
1. operator triggers export(out_path, nonce)
2. snapshotd consults catalog of allow-listed kinds
3. for each kind k: ask storaged for the latest record body
4. assemble SnapshotEnvelope (excluding signature)
5. compute snapshot_digest
6. ask attestd to sign SnapshotDigestDomain(snapshot_digest)
7. write SnapshotEnvelope { signature } to out_path
8. audit: SnapshotExported { records, snapshot_digest }
```

### 7.2 Import pipeline

```text
1. operator triggers import(in_path, peer_identity, nonce)
2. parse envelope; reject TooLarge / malformed
3. recompute snapshot_digest; reject DigestMismatch
4. verify signature using peer_identity.attestation_pubkey
5. evaluate peer policy via identityd; reject IdentityNotPermitted
6. for each record:
       - reject UnknownRecordKind
       - if RollbackRecord: ratchet (never lower min_counter)
       - if MeasurementHeadSummary: append into local audit
       - if NodeIdentityRecord (own): refuse — never overwrite own identity
       - if other: storaged.append with kind-specific dedup
7. on any record-level error, abort atomically; nothing persisted.
8. on success: storaged.append SnapshotImportRecord { snapshot_id, source_id }
9. audit: SnapshotImported { snapshot_id, records, source_node }
```

### 7.3 Replay protection

storaged maintains a SnapshotImported log keyed by
`snapshot_id = SHA256(snapshot_digest)`. Re-importing the same snapshot:

```text
- if not present: apply as new.
- if present: refuse with SnapshotImportError::Replay.
```

### 7.4 Anti-rollback ratchet

For each imported RollbackRecord:

```text
peer_min = peer record.min_counter
local_min = storaged.latest(channel).min_counter
if peer_min < local_min:
    record-level error: RollbackViolation
else:
    storaged.append RollbackRecord { min_counter = max(peer_min, local_min),
                                     source = AdvanceSource::SnapshotImport }
```

A new `AdvanceSource::SnapshotImport = 0x04` is added to RFC v0.3-003's
enum.

---

## 8. Security Design

### 8.1 Threat model deltas

```text
Threat T-170: Adversary forges a snapshot with valid-looking records.
Mitigation:  signature verification against pinned peer identity;
             SnapshotDigestDomain prevents cross-protocol replay.

Threat T-171: Adversary replays a stale snapshot to revert state.
Mitigation:  snapshot_id replay protection + rollback ratchet on
             RollbackRecord.

Threat T-172: Importer accepts a snapshot from an untrusted peer.
Mitigation:  identityd policy evaluation; default SameFamily mode.

Threat T-173: Snapshot smuggles a malicious record kind.
Mitigation:  allow-list at parse time; unknown kinds rejected.

Threat T-174: Importer overwrites its own NodeIdentity from a snapshot.
Mitigation:  hard rule: own-identity records are refused.

Threat T-175: Partial import leaves state inconsistent.
Mitigation:  atomic apply — storaged uses a per-import transaction (a
             single batch append at the end).

Threat T-176: Snapshot grows unbounded and exhausts storage.
Mitigation:  SnapshotEnvelope size capped at MAX_SNAPSHOT_BYTES (1 MiB);
             record_count capped at MAX_SNAPSHOT_RECORDS (256).
```

### 8.2 Audit emission

```text
SnapshotExported       { snapshot_digest, record_count }
SnapshotImported       { snapshot_id, source_node_id_first8, records_applied }
SnapshotImportRefused  { error_code, source_node_id_first8 }
SnapshotImportRollbackViolation { channel_id, peer, local }
```

---

## 9. Memory / Resource Design

- Snapshot envelope ≤ 1 MiB; parsed structure in stack/scratch.
- SnapshotImported log: 16 B id + 8 B tick = 24 B per import; bounded.

---

## 10. Compatibility and Migration

- New service `snapshotd`; new cap kind `Snapshot` with rights `EXPORT`,
  `IMPORT`.
- New AdvanceSource variant.
- No change to existing record formats.

---

## 11. Test Strategy

### 11.1 Host unit tests

```text
- snapshot_envelope_serialise_round_trip
- snapshot_digest_covers_records
- snapshot_signature_round_trip
- snapshot_replay_protection
- snapshot_unknown_kind_rejected
- snapshot_own_identity_rejected
- snapshot_rollback_ratchet_no_lower
- snapshot_atomic_failure_rolls_back
- snapshot_size_cap_enforced
- snapshot_record_count_cap_enforced
```

### 11.2 QEMU smoke

```text
- SMOKE:SNAP:EXPORT_AND_IMPORT_LOOPBACK
- SMOKE:SNAP:CROSS_NODE_HAPPY_PATH
```

### 11.3 Negative

| Marker                                                       | Profile  |
|--------------------------------------------------------------|----------|
| `NEG:SNAP:SIGNATURE_FAILED_REJECTED`                         | snapshot |
| `NEG:SNAP:UNKNOWN_PEER_REJECTED`                             | snapshot |
| `NEG:SNAP:REPLAY_REJECTED`                                   | snapshot |
| `NEG:SNAP:ROLLBACK_VIOLATION_REJECTED`                       | snapshot |
| `NEG:SNAP:UNKNOWN_KIND_REJECTED`                             | snapshot |
| `NEG:SNAP:OWN_IDENTITY_OVERWRITE_REJECTED`                   | snapshot |
| `NEG:SNAP:OVERSIZE_REJECTED`                                 | snapshot |
| `NEG:SNAP:PARTIAL_FAILURE_NO_PERSISTENCE`                    | snapshot |

---

## 12. Acceptance Criteria

```text
- snapshotd binary exists.
- fjell-snapshot-format crate with envelope types.
- ≥ 10 host unit tests pass.
- 2 SMOKE + 8 NEG markers green.
- Cross-node QEMU smoke covers a 2-instance setup.
- ADR-v0.7-002 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/architecture/v0.7-002-snapshot.md
docs/src/format/snapshot-envelope.md
docs/src/operator/snapshot-cli.md
docs/src/adr/v0.7-002-snapshot-allow-list.md
docs/src/adr/v0.7-002-atomic-import.md
```

---

## 14. Open Questions

1. **Encryption at rest of snapshot files** — snapshots contain no
   secrets; integrity is sufficient. If a future use case needs
   confidentiality, wrap the file in a sealed envelope (provider-bound).
2. **Diff format** — large fleets will want delta snapshots. v0.8 RFC
   covers this on top of v0.7's full-snapshot baseline.
3. **MeasurementHeadSummary recompute** — the importer cannot recompute
   the peer's measurement chain locally. Acceptable: the imported summary
   is advisory and never authoritative for the local boot path.

---

## 15. Release Gate (RFC-local)

```text
- snapshotd ships.
- 10 host + 2 SMOKE + 8 NEG markers green.
- ADRs Accepted.
```
