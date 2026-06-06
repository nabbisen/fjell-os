# RFC-v0.8-005: Policy Governance and Fleet Policy Distribution

**Status.** Implemented (v0.8.0)

## Status

Draft (revised, supersedes pack v0.8-005 draft)

## Target Version

`v0.8.0`.

## Phase

Fleet Operations Plane — Epic E (Policy Distribution).

## Related Work

- v0.2 `cap-broker` policy bundle format.
- v0.3 RFC 002 — `KeyPurpose::PolicyVerification`.
- v0.7 RFC 003 — MeasurementAuditPolicy (one specific policy type).
- v0.8 RFC 001 — FleetRoster.
- v0.8 RFC 003 — RolloutPlan (consumes node-side policy gating).

---

## 1. Summary

Unify Fjell's various policy bundles (cap-broker rules, audit policy,
identity policy, rollout gating, recovery role gating) into a single
**signed FleetPolicy** envelope with a stable shape: an array of typed
policy sections, each carrying its own canonical digest and a global
envelope signature.

Define the distribution and acceptance rules: pull-only, monotonic
`policy_epoch`, atomic acceptance, no partial application.

---

## 2. Motivation

By v0.8 the system has accumulated several signed bundles distributed
through different paths and verified against different keyring purposes.
This creates seams:

- different replay-protection conventions;
- different rollback rules;
- different operator UX.

A single envelope means:

- one verification path;
- one set of audit events;
- one operator command to roll out fleet-wide policy.

---

## 3. Goals

```text
- One FleetPolicy envelope carrying up to N typed sections.
- Each section's canonical digest part of the envelope digest.
- Envelope signed via KeyPurpose::PolicyVerification with
  domain "FJELL-FLEET-POLICY-SIGN-V1".
- Acceptance is atomic: either every section's digest checks and parses
  or none is applied.
- policy_epoch monotonic; lower-epoch envelopes rejected.
- Each section type has a stable section_tag and a frozen-schema entry
  (RFC v0.6-003).
- Distribution: pull-only via secure-transportd Diagnostics channel.
```

## 4. Non-Goals

```text
- No partial application. The envelope is the unit of trust.
- No per-section rotation independent of the envelope.
- No automatic conflict resolution between policies of the same type at
  different epochs — only the latest envelope is authoritative.
- No remote write or removal of sections; envelope replacement only.
```

---

## 5. External Design

### 5.1 FleetPolicy envelope

```rust
pub const FLEET_POLICY_VERSION: u16 = 1;
pub const MAX_POLICY_SECTIONS:   usize = 16;

pub struct FleetPolicyEnvelope {
    pub schema_version: u16,
    pub fleet_id:       [u8; 16],
    pub policy_epoch:   u32,
    pub issued_tick:    u64,
    pub section_count:  u8,
    pub sections:       [PolicySection; MAX_POLICY_SECTIONS],
    pub envelope_digest: Digest32,
}

pub struct PolicySection {
    pub section_tag:    u16,        // identifies the section type, see §6.1
    pub schema_version: u16,
    pub body_len:       u32,
    pub body_digest:    Digest32,   // digest over the body bytes
    pub body_offset:    u32,        // offset within envelope payload area
}

pub struct SignedFleetPolicyEnvelope {
    pub envelope:  FleetPolicyEnvelope,
    pub body_bytes: ArrayVec<u8, 16384>,
    pub signature: Signature,
}
```

The `body_bytes` is the concatenated payload; `body_offset` + `body_len`
locate each section's bytes.

### 5.3 Operator workflow

```text
$ fjell-fleet-tool policy build \
       --capbroker capbroker.bin \
       --audit     audit-policy.bin \
       --identity  ident-policy.bin \
       --recovery  recovery-roles.bin \
       --rollout   rollout-defaults.bin \
       --epoch     7 \
       --out       fleet-policy-v7.bin
$ fjell-fleet-tool policy sign --key fleet-root.key
$ fjell-fleet-tool policy publish
```

---

## 6. Data Model

### 6.1 Section tags

```text
0x0010  CapBrokerPolicy            — rights table per service / scope
0x0020  IdentityPolicy             — RFC v0.7-001 NodeIdentityPolicy
0x0030  MeasurementAuditPolicy     — RFC v0.7-003
0x0040  RolloutDefaults            — fallback waves / floors
0x0050  RecoveryRolePolicy         — which roles may issue which actions
0x0060  SemanticEmitFilter         — additional intent filters (advisory)
0x00FF  Reserved
```

Adding a section tag requires an ADR; the section's frozen-schema file is
created at the same time.

### 6.2 Canonical envelope digest

```text
envelope_digest = SHA256(
    "FJELL-FLEET-POLICY-V1" ||
    schema u16 LE || fleet_id 16 B || policy_epoch u32 LE ||
    issued_tick u64 LE || section_count u8 ||
    for each section:
        section_tag u16 LE || schema_version u16 LE ||
        body_len u32 LE || body_digest 32 B
)
```

`body_digest` for each section is computed independently before envelope
assembly. The envelope digest does **not** include the body bytes — only
their digests — which lets a verifier skip a section it already has
cached (by `body_digest`).

### 6.3 Persistent acceptance record

```rust
pub struct PolicyAcceptanceRecord {
    pub schema_version: u16,
    pub fleet_id:       [u8; 16],
    pub policy_epoch:   u32,
    pub envelope_digest: Digest32,
    pub accepted_at_tick: u64,
    pub section_count:  u8,
    pub section_digests: [Digest32; MAX_POLICY_SECTIONS],
    pub record_digest:  Digest32,
}
```

StoreRecordKind `PolicyAcceptance = 0x1A`.

---

## 7. Internal Design

### 7.1 Acceptance pipeline

```text
on receive SignedFleetPolicyEnvelope:
  if envelope.fleet_id != self.fleet_id → drop (audit)
  recompute envelope_digest; reject mismatch
  verify signature using KeyPurpose::PolicyVerification anchor
  if envelope.policy_epoch <= cached.policy_epoch → reject Stale

  // pre-flight every section before applying any
  for each section in envelope.sections:
      if section.section_tag unknown → reject UnknownSection
      slice = body_bytes[section.body_offset .. + section.body_len]
      if SHA256(slice) != section.body_digest → reject BodyDigestMismatch
      validate slice against section's frozen schema; reject on parse error

  // apply atomically: in-memory snapshot, then commit
  new_state = snapshot(current_state)
  for each section: new_state.apply(section_tag, slice)
  storaged.append PolicyAcceptanceRecord(...)
  current_state = new_state
  emit FleetPolicyApplied { policy_epoch, envelope_digest }
```

Any failure in the pre-flight stage leaves `current_state` untouched.

### 7.2 Per-section appliers

Each section type has its own applier living in the owning service:

- `CapBrokerPolicy` → cap-broker
- `IdentityPolicy`  → identityd
- `MeasurementAuditPolicy` → measuredd / summaryd
- `RolloutDefaults` → fleet-agent (rollout gating)
- `RecoveryRolePolicy` → recoveryd
- `SemanticEmitFilter` → semantic-stream

`fleet-agent` is the broker that distributes parsed sections to each
service via IPC.

### 7.3 Replay of acceptance records

storaged scan picks the highest `policy_epoch` per `fleet_id`. The
authority service can compute the current policy state from the section
digests; bodies are pulled from cache or re-fetched.

### 7.4 Section caching

`fleet-agent` keeps a cache of `body_digest → body_bytes` for the last 4
envelopes. If an envelope arrives where some section bodies match cached
digests, the bodies need not be re-transmitted (transport-side optimisation
deferred to v0.9).

---

## 8. Security Design

### 8.1 Threat model deltas

```text
Threat T-240: Adversary substitutes one section's body bytes while
              keeping the envelope digest.
Mitigation:  envelope digest covers body_digest; mutated bytes change
              body_digest; envelope digest no longer matches.

Threat T-241: Older policy epoch replayed.
Mitigation:  monotonic policy_epoch per fleet_id; lower rejected.

Threat T-242: Partial application leaves cap-broker tightened but
              audit policy widened.
Mitigation:  atomic acceptance — all sections validate before any
              applier runs.

Threat T-243: Unknown section type smuggled in.
Mitigation:  reader rejects unknown section_tag (UnknownSection).

Threat T-244: Adversary controls the secure-transportd channel and
              suppresses new envelopes.
Mitigation:  node can also accept an envelope pulled out-of-band
              (operator-side import); freshness is operator concern.

Threat T-245: Cross-fleet policy delivered to wrong fleet.
Mitigation:  fleet_id covered by digest; mismatched fleet_id rejected.

Threat T-246: Excessively large envelope ties up parser.
Mitigation:  body_bytes capped at 16 KiB; section_count capped at
              MAX_POLICY_SECTIONS = 16; section body_len capped at
              MAX_SECTION_BODY = 4 KiB.
```

### 8.2 Audit emission

```text
FleetPolicyReceived          { policy_epoch, section_count }
FleetPolicyRejected          { policy_epoch, error_code, failing_section_tag }
FleetPolicyApplied           { policy_epoch, envelope_digest }
FleetPolicySectionApplied    { section_tag, body_digest }
```

---

## 9. Memory / Resource Design

- Envelope ≤ 16 KiB body + ~600 B header.
- Cache 4 envelopes ≈ 70 KiB.
- PolicyAcceptanceRecord ≈ 600 B.

---

## 10. Compatibility and Migration

- Existing per-service policy bundles continue to work for nodes that are
  *not* enrolled. Enrolled nodes ignore standalone bundles in favour of
  the fleet envelope.
- A migration ADR enumerates how the v0.5 audit policy (RFC v0.7-003)
  bundle bytes map into section_tag 0x0030.
- `fleet-agent` is the only path that applies sections; standalone
  service tools are deprecated for enrolled nodes (kept for offline /
  recovery).

---

## 11. Test Strategy

### 11.1 Host unit tests

```text
- envelope_digest_covers_section_digests
- envelope_digest_excludes_body_bytes
- section_body_digest_recompute_check
- section_unknown_tag_rejected
- envelope_stale_epoch_rejected
- envelope_wrong_fleet_rejected
- envelope_body_oversize_rejected
- section_body_oversize_rejected
- atomic_application_rolls_back_on_any_failure
- cache_hit_on_unchanged_section_digest
```

### 11.2 QEMU smoke

```text
- SMOKE:POLICY:APPLY_FIVE_SECTIONS
- SMOKE:POLICY:STALE_EPOCH_REJECTED
- SMOKE:POLICY:ATOMIC_FAILURE_NO_CHANGE
```

### 11.3 Negative

| Marker                                                  | Profile  |
|---------------------------------------------------------|----------|
| `NEG:POLICY:ENVELOPE_DIGEST_MISMATCH_REJECTED`          | policy   |
| `NEG:POLICY:UNKNOWN_SECTION_REJECTED`                   | policy   |
| `NEG:POLICY:STALE_EPOCH_REJECTED`                       | policy   |
| `NEG:POLICY:WRONG_FLEET_REJECTED`                       | policy   |
| `NEG:POLICY:BODY_DIGEST_MISMATCH_REJECTED`              | policy   |
| `NEG:POLICY:OVERSIZE_ENVELOPE_REJECTED`                 | policy   |
| `NEG:POLICY:PARTIAL_FAILURE_ATOMIC_ROLLBACK`            | policy   |

---

## 12. Acceptance Criteria

```text
- fjell-fleet-policy-format crate lands with envelope + section types.
- fleet-agent integrates envelope acceptance + per-service distribution.
- All 6 section types parseable.
- 10 host tests + 3 SMOKE + 7 NEG markers green.
- Migration ADR enumerates mapping for existing per-service bundles.
- ADR-v0.8-005 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/architecture/v0.8-005-fleet-policy.md
docs/src/format/fleet-policy-envelope.md
docs/src/format/policy-sections.md
docs/src/operator/policy-cli.md
docs/src/adr/v0.8-005-section-tags.md
docs/src/adr/v0.8-005-atomic-application.md
docs/src/adr/v0.8-005-deprecation-of-standalone-bundles.md
```

---

## 14. Open Questions

1. **Differential envelopes** — current design re-transmits all section
   bodies. With `body_digest` covered, a differential transmission is
   safe; deferred to v0.9 as a transport optimisation.
2. **Per-cohort policy** — different cohorts with different cap-broker
   rules. Out of scope for v0.8.0; possible via multiple FleetPolicy
   envelopes keyed by cohort_mask in a future RFC.
3. **Section retirement** — removing a section type. Currently a
   `BREAKING-SCHEMA` policy change requiring ADR; appropriate friction.

---

## 15. Release Gate (RFC-local)

```text
- Envelope format + fleet-agent integration + section appliers in 6
  services.
- 10 host + 3 SMOKE + 7 NEG markers green.
- ADRs Accepted.
- CHANGELOG entries filed.
```
