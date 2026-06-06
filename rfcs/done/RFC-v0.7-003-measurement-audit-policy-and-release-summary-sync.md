# RFC-v0.7-003: Measurement Audit Policy and Release Summary Sync

**Status.** Implemented (v0.7.0)

## Status

Draft (revised, supersedes pack v0.7-003 draft)

## Target Version

`v0.7.0`.

## Phase

Distributed Snapshot Sync — Epic C (Measurement / Release Summary).

## Related Work

- v0.7 RFC 001 (NodeIdentity), v0.7 RFC 002 (Snapshot).
- v0.4 RFC 004 (UpdateIndex; release-side counterpart).
- v0.4 RFC 005 (DiagnosticBundle; same redaction philosophy).

---

## 1. Summary

Define **`MeasurementSummary`** and **`ReleaseSummary`** — two compact,
audit-grade summaries that one Fjell node can publish to another. Define
the *measurement audit policy* that controls which measurement events are
projected into the summary and which are not.

Both summaries are signed, idempotent, and bounded. They are the standard
unit of trust exchange between nodes when a full snapshot is too large
or too revealing.

---

## 2. Motivation

A full snapshot (RFC v0.7-002) is heavy. For fleet workflows where one
node wants only "what's the current measurement / release state of node
X", a lightweight summary is more appropriate:

- the diagnostic posture without the entire log;
- the release counters without the staging history;
- enough to make a `same/different` decision.

The audit policy ensures summaries can't accidentally leak more than
intended.

---

## 3. Goals

```text
- MeasurementSummary: signed digest tree projection of the chain head plus
  a small bounded "recent kinds" tally.
- ReleaseSummary: per-channel counter + active anchor epoch + last commit
  tick.
- Idempotent: regenerating produces byte-identical output if state hasn't
  changed.
- Bounded size (≤ 4 KiB).
- Strict allow-list of measurement-kinds projected into summary; ADR-gated.
```

## 4. Non-Goals

```text
- No streaming. Summaries are point-in-time.
- No automatic cadence. Operator or fleet-agent triggers.
- No partial summaries.
- No claim of "current health" beyond the explicitly enumerated kinds.
```

---

## 5. External Design

### 5.1 MeasurementSummary

```rust
pub const MEASUREMENT_SUMMARY_VERSION: u16 = 1;
pub const MAX_KIND_COUNTS: usize = 16;

pub struct MeasurementSummary {
    pub schema_version:        u16,
    pub source_node_id:        NodeId,
    pub issued_tick:           u64,
    pub head_seq:              u64,
    pub head_chain_digest:     Digest32,
    pub kind_count:            u8,
    pub kind_counts:           [KindCount; MAX_KIND_COUNTS],
    pub policy_digest:         Digest32,    // hash of the audit policy in effect
    pub summary_digest:        Digest32,
}

pub struct KindCount {
    pub kind: u8,           // MeasurementKind tag
    pub count: u32,
}

pub struct SignedMeasurementSummary {
    pub summary:    MeasurementSummary,
    pub signature:  Signature,
    pub signed_at_epoch: u32,
}
```

### 5.2 ReleaseSummary

```rust
pub const RELEASE_SUMMARY_VERSION: u16 = 1;
pub const MAX_RELEASE_CHANNELS: usize = 4;

pub struct ReleaseSummary {
    pub schema_version:        u16,
    pub source_node_id:        NodeId,
    pub issued_tick:           u64,
    pub channel_count:         u8,
    pub channels:              [ChannelStatus; MAX_RELEASE_CHANNELS],
    pub summary_digest:        Digest32,
}

pub struct ChannelStatus {
    pub channel_id:        [u8; 8],
    pub current_counter:   u64,
    pub min_counter:       u64,
    pub active_anchor_epoch: u32,
    pub last_confirm_tick: u64,
    pub last_advance_source: u8,    // AdvanceSource tag
}
```

### 5.3 Audit policy

```rust
pub struct MeasurementAuditPolicy {
    pub schema_version: u16,
    pub allowed_kinds:  [u8; 16],
    pub allowed_kind_count: u8,
    pub max_count_per_kind: u32,        // cap on count field
    pub policy_digest:  Digest32,
}
```

`MeasurementAuditPolicy` is a signed configuration bundle distributed via
`configd`; loading it requires `KeyPurpose::PolicyVerification`. The
policy_digest is bound into every `MeasurementSummary` so verifiers know
*which* policy filtered the contents.

---

## 6. Data Model

### 6.1 Canonical digests

```text
summary_digest (MeasurementSummary) = SHA256(
    "FJELL-MSUMMARY-V1" ||
    schema u16 LE ||
    source_node_id 16 B ||
    issued_tick u64 LE ||
    head_seq u64 LE ||
    head_chain_digest 32 B ||
    kind_count u8 ||
    for each kind: kind u8 || count u32 LE ||
    policy_digest 32 B
)
```

```text
summary_digest (ReleaseSummary) = SHA256(
    "FJELL-RSUMMARY-V1" ||
    schema u16 LE ||
    source_node_id 16 B ||
    issued_tick u64 LE ||
    channel_count u8 ||
    for each channel:
        channel_id 8 B || current u64 LE || min u64 LE ||
        active_anchor_epoch u32 LE || last_confirm_tick u64 LE ||
        last_advance_source u8
)
```

### 6.2 Signing domain

Both summaries are signed via attestd with domain separators:

```text
sign input = SHA256("FJELL-MSUMMARY-SIGN-V1" || summary_digest)
sign input = SHA256("FJELL-RSUMMARY-SIGN-V1" || summary_digest)
```

This prevents cross-protocol replay against attestation records or
snapshots.

---

## 7. Internal Design

### 7.1 Summary builders (in `summaryd`, a new service)

```text
on build_measurement_summary():
  policy = configd.load("/cfg/audit-policy.bin")
  head = measuredd.head()
  kinds = [(k, count) for k,count in measuredd.tally_by_kind()
             if k in policy.allowed_kinds]
  kinds = kinds[:MAX_KIND_COUNTS]
  cap each count at policy.max_count_per_kind
  build MeasurementSummary
  ask attestd to sign with MSUMMARY-SIGN-V1 domain
```

```text
on build_release_summary():
  for each channel in known channels (up to MAX_RELEASE_CHANNELS):
      r = storaged.latest_rollback_record(channel)
      m = storaged.latest_release_metadata(channel)
      a = keyring.active_epoch(ReleaseVerification)
      build ChannelStatus
  build ReleaseSummary
  ask attestd to sign
```

### 7.2 Determinism

Both builders are pure functions of (state, policy). Two consecutive calls
on identical state produce identical signed bytes (modulo `issued_tick`
which monotonically increases; tests use a fixed-tick mode).

### 7.3 Distribution

Summaries can be:

- exported as files (`fjell-tools summary measurement export -o foo.msum`);
- pushed via `secure-transportd`'s `Diagnostics` channel (extending v0.4
  RFC 005);
- embedded inside a `Snapshot` (RFC v0.7-002).

### 7.4 Verifier API

```rust
pub fn verify_measurement_summary(
    bytes: &[u8],
    peer_identity: &SignedNodeIdentity,
    known_policies: &[MeasurementAuditPolicy],
) -> Result<MeasurementSummary, SummaryError>;

pub fn verify_release_summary(
    bytes: &[u8],
    peer_identity: &SignedNodeIdentity,
) -> Result<ReleaseSummary, SummaryError>;
```

`verify_measurement_summary` requires the verifier to *recognise* the
policy by digest. Unknown-policy summaries are rejected with
`SummaryError::UnknownPolicy` — a peer can't quietly use a custom policy.

---

## 8. Security Design

### 8.1 Threat model deltas

```text
Threat T-180: Peer publishes inflated kind_counts to fake activity.
Mitigation:  max_count_per_kind cap; the verifier's known policy bounds
             the maximum; out-of-range rejected.

Threat T-181: Peer publishes summary signed by a wrong purpose.
Mitigation:  signature is over MSUMMARY-SIGN-V1 / RSUMMARY-SIGN-V1
             domains; attestd refuses to sign without an attestation
             keyring epoch active.

Threat T-182: Outdated summary replayed.
Mitigation:  issued_tick is in the digest; consumers compare against
             previous summary's tick from same node; older rejected.

Threat T-183: Policy substitution — peer claims a benign policy was used
             but actually used a permissive one.
Mitigation:  policy_digest is bound into summary_digest; verifier checks
             the digest matches a known-allowed policy in its registry.

Threat T-184: Release summary lowers an apparent counter to confuse
             rollback decisions.
Mitigation:  release summaries are advisory; they never modify local
             min_counter (only snapshots can, RFC v0.7-002, with ratchet).
```

### 8.2 Audit emission

```text
SummaryMeasurementBuilt     { summary_digest, policy_digest }
SummaryReleaseBuilt         { summary_digest, channel_count }
SummaryVerifiedOk           { kind, summary_digest, source_node_id_first8 }
SummaryVerifyFailed         { kind, error_code }
SummaryUnknownPolicy        { policy_digest }
```

---

## 9. Memory / Resource Design

- MeasurementSummary ≈ 200 B; ReleaseSummary ≈ 230 B.
- summaryd cache: last 2 of each = ≈ 1 KiB.

---

## 10. Compatibility and Migration

- New crate `fjell-summary-format`.
- New service `summaryd`.
- New configd path `/cfg/audit-policy.bin`.

---

## 11. Test Strategy

### 11.1 Host unit tests

```text
- m_summary_digest_covers_policy_digest
- m_summary_digest_covers_kind_counts
- r_summary_digest_covers_per_channel_fields
- m_summary_excludes_unallowed_kinds
- m_summary_caps_kind_counts
- m_summary_idempotent_under_fixed_tick
- r_summary_idempotent_under_fixed_tick
- summary_signature_round_trip
- summary_unknown_policy_rejected
- summary_replay_older_tick_rejected
```

### 11.2 QEMU smoke

```text
- SMOKE:SUMMARY:M_BUILD_AND_VERIFY_LOOPBACK
- SMOKE:SUMMARY:R_BUILD_AND_VERIFY_LOOPBACK
- SMOKE:SUMMARY:CROSS_NODE_VERIFY
```

### 11.3 Negative

| Marker                                                  | Profile  |
|---------------------------------------------------------|----------|
| `NEG:SUMMARY:UNKNOWN_POLICY_REJECTED`                   | summary  |
| `NEG:SUMMARY:SIGNATURE_FAILED_REJECTED`                 | summary  |
| `NEG:SUMMARY:OLDER_TICK_REJECTED`                       | summary  |
| `NEG:SUMMARY:CROSS_PROTOCOL_REPLAY_REJECTED`            | summary  |
| `NEG:SUMMARY:KIND_OUT_OF_POLICY_REJECTED`               | summary  |
| `NEG:SUMMARY:COUNT_OVER_POLICY_CAP_REJECTED`            | summary  |

---

## 12. Acceptance Criteria

```text
- fjell-summary-format crate ships with both types.
- summaryd binary exists.
- MeasurementAuditPolicy bundle format defined; configd loads it.
- 10 host tests + 3 SMOKE + 6 NEG markers green.
- ADR-v0.7-003 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/architecture/v0.7-003-summary-sync.md
docs/src/format/measurement-summary.md
docs/src/format/release-summary.md
docs/src/format/audit-policy.md
docs/src/adr/v0.7-003-summary-policy-binding.md
```

---

## 14. Open Questions

1. **Streaming summaries** — current design is one-shot. Streaming would
   be useful for very large fleets; deferred to v0.8.
2. **Differential summaries** — diff against a known prior summary,
   reducing wire size. Deferred to v0.8.
3. **Multiple policies per node** — current design has one active audit
   policy. If a node needs to publish to multiple audiences with different
   policies, that's a v0.8 concern.

---

## 15. Release Gate (RFC-local)

```text
- summaryd ships.
- 10 host + 3 SMOKE + 6 NEG green.
- ADR Accepted.
```
