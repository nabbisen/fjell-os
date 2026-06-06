# RFC-v0.4-005: Remote Diagnostics and Attestation Transport Foundation

**Status.** Implemented (v0.4.0)

## Status

Draft (revised, supersedes pack v0.4-005 draft)

## Target Version

`v0.4.0`.

## Phase

Minimal Secure Control-Plane Networking — Epic E (Diagnostics & Remote Attest).

## Related Work

- v0.3 RFC 004 — `AttestationRecordV2` (the payload shape).
- v0.4 RFC 003 — `secure-transportd` (the transport).
- v0.7 RFC 003 — measurement-audit policy and release-summary sync.
- v0.8 RFC 004 — remote diagnostics and recovery intent (extends this RFC).

---

## 1. Summary

Add two transport-bound flows on top of `secure-transportd`:

- **DiagnosticsPush** — a one-way push of bounded, **typed** diagnostic
  records (audit events, semantic intents, measurement chain head) to a
  pinned diagnostics endpoint;
- **AttestationPush** — a request/response exchange that pushes a
  `SignedAttestationRecordV2` and receives a server `Nonce` used for the next
  push (rolling challenge).

Both flows preserve Fjell's "operator-initiated, not background" rule.

Introduce `diagnosticsd`, a small service that collects records from
`auditd`/`measuredd`/`semantic-stream` into a *redacted, schema-versioned*
diagnostic bundle.

---

## 2. Motivation

A device without remote-observable trust evidence cannot participate in a
fleet, even minimally. v0.4 needs the *foundation* — the wire shapes and
typed channels — even though full fleet management is v0.8.

The bound on what is sent is structural, not editorial:

- only enumerated record kinds are eligible;
- only fields explicitly listed in a schema are projected;
- nothing payload-like, nothing free-form.

---

## 3. Goals

```text
- Define DiagnosticBundle: a typed, versioned blob with audit events,
  semantic intents, measurement-head, and last attestation digest only.
- Define AttestationExchange: typed request/response over the Attestation
  ChannelKind.
- Implement diagnosticsd as the only service permitted to construct
  DiagnosticBundle (cap-broker enforced).
- Push is operator-initiated; no autonomous push.
- Server-side challenge nonce is the only source of freshness; recording is
  in the AttestationRecordV2 freshness claim.
- All redaction rules are unit-tested.
```

## 4. Non-Goals

```text
- No log streaming (only finalized bundles).
- No PII handling (Fjell has no PII; all redaction rules assume "fail-closed
  unless on allow-list").
- No background or scheduled push.
- No autonomous retry policy (operator decides).
- No fleet identity (v0.7).
```

---

## 5. External Design

### 5.1 Operator workflow

```text
$ fjell-tools diag bundle             # build bundle and show summary
$ fjell-tools diag push <endpoint>    # push to pinned diagnostics endpoint
$ fjell-tools attest push <endpoint>  # push current attestation
$ fjell-tools attest challenge        # show the cached server nonce
```

### 5.2 Bundle shape

```rust
pub const DIAG_BUNDLE_VERSION: u16 = 1;
pub const MAX_AUDIT_EVENTS:    usize = 64;
pub const MAX_SEMANTIC_INTENTS: usize = 32;

pub struct DiagnosticBundle {
    pub schema_version:        u16,
    pub bundle_id:             [u8; 8],
    pub created_tick:          u64,
    pub provider_id:           TrustProviderId,
    pub keyring_anchor_epoch:  u32,
    pub measurement_head:      Digest32,
    pub last_attestation:      Digest32,
    pub audit_event_count:     u8,
    pub audit_events:          [DiagAuditEvent; MAX_AUDIT_EVENTS],
    pub semantic_intent_count: u8,
    pub semantic_intents:      [DiagIntent; MAX_SEMANTIC_INTENTS],
    pub bundle_digest:         Digest32,
}

pub struct DiagAuditEvent {
    pub seq:        u32,
    pub kind_tag:   u16,         // one of the allow-listed kinds (see §6.2)
    pub code:       u16,         // reason / error code if applicable
    pub at_tick:    u64,
}

pub struct DiagIntent {
    pub seq:        u32,
    pub intent_tag: u16,
    pub at_tick:    u64,
    pub code:       u16,
}
```

### 5.3 Attestation exchange

```text
client → server:  SignedAttestationRecordV2 (signed with server's last nonce
                  bound into freshness)
server → client:  16-byte next_nonce (to be used in the next push)
```

The client caches `next_nonce` in storaged. If absent (first push), client
uses a `NonceClass::LocalOnly` nonce; the server responds with the first
remote nonce.

---

## 6. Data Model

### 6.1 Allow-listed audit event kinds (subset projected to bundle)

```text
0x0010 KernelBootBanner
0x0020 ServiceManagerReady
0x0040 TrustProviderRegistered
0x0041 TrustProviderFaulted
0x0050 KeyringActiveEpochAdvanced
0x0060 UpgradeStateTransition
0x0070 UpgradeRollbackRejected
0x0080 BootRollbackBlockedSlot
0x0090 AttestationRecordSigned
0x0091 AttestationVerifyFailed
0x00A0 NetDriverFaulted
0x00A1 SxtCertVerifyFailed
0x00A2 SxtHandshakeFailed
0x00B0 RecoveryEntered
```

Anything outside this list is dropped from the bundle.

### 6.2 Allow-listed semantic intent tags (subset)

```text
0x0100 UPDATE.STAGING_STARTED
0x0101 UPDATE.STAGING_ADVANCED
0x0102 UPDATE.STAGING_FAILED
0x0103 UPDATE.STAGING_CONFIRMED
0x0110 UPDATE.ROLLBACK_BLOCKED
0x0120 ATTEST.RECORD_SIGNED
0x0130 SECURITY.REGISTRY_ENFORCING
0x0140 NET.LINK_DOWN
0x0150 RECOVERY.ENTERED
```

### 6.3 Redaction rules

- **No payload bytes** — only kinds, codes, seqs, ticks.
- **No file paths** — recovery events drop path fields.
- **No service-private identifiers** — only tags from the enumerations.
- **All variable-length strings dropped.** If a field is variable-length, it
  is omitted; bundle is fixed-shape only.

Redaction is encoded in the bundle-builder, not in the source services. The
builder is the trust boundary.

### 6.4 Canonical digest

```text
bundle_digest = SHA256(
    "FJELL-DIAG-V1" ||
    schema u16 LE ||
    bundle_id 8 B ||
    created_tick u64 LE ||
    provider_id u32 LE ||
    keyring_anchor_epoch u32 LE ||
    measurement_head 32 B ||
    last_attestation 32 B ||
    audit_event_count u8 ||
    for each audit event: seq u32 LE || kind u16 LE || code u16 LE || at_tick u64 LE ||
    semantic_intent_count u8 ||
    for each intent: seq u32 LE || tag u16 LE || at_tick u64 LE || code u16 LE
)
```

The push transmits the canonical bytes + the bundle_digest separately so the
server can check digest integrity without re-encoding.

---

## 7. Internal Design

### 7.1 diagnosticsd flow

```text
on operator bundle():
  cap_required: DIAG_BUILD
  collect from auditd: last MAX_AUDIT_EVENTS audit events
  filter by allow-list
  collect from semantic-stream: last MAX_SEMANTIC_INTENTS intents
  filter by allow-list
  read measurement head from measuredd
  read last attestation digest from attestd
  build DiagnosticBundle; compute digest
  return bundle to operator
```

```text
on operator push(endpoint):
  cap_required: DIAG_PUSH + ChannelCap{Diagnostics, SXT_RPC_DIAG}
  open channel via secure-transportd
  serialise bundle (canonical bytes) + digest
  send SXT_DIAG_PUSH
  receive SXT_DIAG_ACK
  audit: DiagnosticBundlePushed { bundle_id, status }
```

### 7.2 attestd push flow

```text
on operator attest_push():
  cap_required: ATTEST_PUSH + ChannelCap{Attestation, SXT_RPC_ATTEST}
  nonce = storaged.next_remote_nonce.or(local_random)
  bind nonce into freshness; generate v2 record
  open channel via secure-transportd
  send SXT_ATTEST_PUSH
  receive SXT_ATTEST_CHALLENGE { nonce }
  storaged.store_next_remote_nonce(nonce)
  audit: AttestationPushed { record_id, server_provided_nonce_bytes }
```

### 7.3 Bundle-size hard cap

`DiagnosticBundle` worst-case packed: 16 + 64 × 16 + 32 × 16 ≈ 1.6 KiB.
This is the on-wire cap. If a future RFC raises the limits, a new
`DIAG_BUNDLE_VERSION` is introduced.

---

## 8. Security Design

### 8.1 Threat model deltas

```text
Threat T-100: Diagnostics push reveals internal state to attacker.
Mitigation:  allow-list + fixed-shape redaction; no variable fields.

Threat T-101: Replay of a stale attestation record.
Mitigation:  rolling nonce; server-supplied nonce becomes the client's next
             freshness; old records' nonces differ.

Threat T-102: Service other than diagnosticsd builds a bundle and bypasses
              redaction.
Mitigation:  cap-broker: only diagnosticsd holds DIAG_BUILD.

Threat T-103: Operator triggers a push that drains the network.
Mitigation:  per-channel byte budget enforced by secure-transportd.

Threat T-104: Server nonce reuse forced by compromised storaged.
Mitigation:  attestd checks last-used-nonce log; reuse rejected by
             AttestationVerifyFailed{NonceReuse}.
```

### 8.2 Audit emission

```text
DiagnosticBundleBuilt       { bundle_id, audit_count, intent_count }
DiagnosticBundlePushed      { bundle_id, status, server_endpoint_hash }
AttestationPushed           { record_id }
AttestationServerNonceStored{ digest_of_nonce }
```

---

## 9. Memory / Resource Design

- DiagnosticBundle worst-case ≈ 1.6 KiB; stack allocation in diagnosticsd.
- One channel per push; not pooled.

---

## 10. Compatibility and Migration

- New ChannelKind discriminants `Diagnostics = 0x02` and
  `Attestation = 0x03` already reserved in RFC v0.4-003.
- New cap rights inside the `Channel` cap kind.
- `storaged` gains one new record kind for the next-remote-nonce.

---

## 11. Test Strategy

### 11.1 Host unit tests (`fjell-diag-format`)

```text
- bundle_digest_covers_all_fields
- bundle_serialise_then_parse_round_trip
- bundle_audit_count_cap_enforced
- bundle_intent_count_cap_enforced
- redaction_drops_unlisted_kind
- redaction_drops_variable_string
- bundle_digest_changes_on_field_mutation
```

### 11.2 QEMU smoke tests

```text
- SMOKE:DIAG:BUILD                   — build bundle, no push
- SMOKE:DIAG:PUSH                    — build + push, server returns ack
- SMOKE:ATTEST:PUSH_AND_CHALLENGE   — push + nonce response
```

### 11.3 QEMU negative tests

| Marker                                                  | Profile |
|---------------------------------------------------------|---------|
| `NEG:DIAG:UNAUTHORISED_BUILD_REJECTED`                  | diag    |
| `NEG:DIAG:UNAUTHORISED_PUSH_REJECTED`                   | diag    |
| `NEG:DIAG:UNKNOWN_KIND_DROPPED`                         | diag    |
| `NEG:ATTEST:NONCE_REPLAY_REJECTED`                      | diag    |
| `NEG:ATTEST:STALE_PROVIDER_ID_REJECTED`                 | diag    |
| `NEG:DIAG:BUNDLE_OVER_LIMIT_REJECTED`                   | diag    |

---

## 12. Acceptance Criteria

```text
- diagnosticsd binary exists; cap-broker rows added.
- fjell-diag-format crate exists with host tests.
- 3 SMOKE + 6 NEG markers green.
- attestd push round-trip works in QEMU.
- ADR-v0.4-005 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/architecture/v0.4-005-remote-diag-attest.md
docs/src/development/v0.4-005-remote-diag-attest.md
docs/src/format/diagnostic-bundle.md
docs/src/adr/v0.4-005-redaction-boundary.md
```

---

## 14. Open Questions

1. **Server-side identity** — v0.4 pins the SNI but does not bind a
   server-side public key independent of TLS. v0.7's fleet identity
   introduces a server-key field; this RFC's `endpoint_hash` audit field is a
   forward-compat placeholder.
2. **Bundle compression** — current shape is fixed and small. Compression
   would weaken the redaction story by changing wire size on content; left
   off.

---

## 15. Release Gate (RFC-local)

```text
- Code merged.
- 3 SMOKE + 6 NEG markers green.
- Redaction unit tests green.
- ADR Accepted.
- CHANGELOG entries filed.
```
