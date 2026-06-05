# RFC-v0.3-004: Local Attestation Profile v2 and Measurement Binding

## Status

Draft (revised, supersedes pack v0.3-004 draft)

## Target Version

`v0.3.0`.

## Phase

Hardware Trust Abstraction — Epic D (Attestation Profile v2).

## Related Work

- v0.3 RFC 001 (`HardwareTrustProvider`); v0.3 RFC 002 (Keyring/KeyEpoch);
  v0.3 RFC 003 (`ReleaseMetadata`, `RollbackRecord`).
- v0.2 existing `attestation-format` crate (`AttestationRecord`,
  `SignedAttestationRecord`).
- v0.4 RFC 005 (remote attestation transport; consumes v2 records).
- v0.7 RFC 003 (`AttestationSummary` sync; consumes v2 digests).

---

## 1. Summary

Define **`AttestationProfile::FjellLocalV2`**: a binary attestation record
schema that *strictly binds* into a single signed digest:

- the trust provider that produced the signature;
- the keyring active epoch at signing time;
- the measurement chain head (chain_digest);
- the boot-control state (selected slot, boot_id);
- the persisted rollback `min_counter` per channel;
- a nonce (caller-supplied freshness);
- the snapshot digest (if any);
- a health summary.

The v1 record (which existed in v0.2) is retained for backward compatibility
during the v0.3 transition and deprecated in v0.3.0-rc.1.

The schema upgrade allows a remote verifier (v0.4) to make a single signature
check sufficient to validate the entire local-trust state.

---

## 2. Motivation

The v1 record signs a digest that includes:

```text
- selected_slot, boot_id, kernel_digest
- release/rootfs/policy verification flags + digests
- measurement head and chain_digest
- snapshot id + digest
- health target + status
- freshness (generation, key_epoch, status)
```

That covers most of the picture but has three gaps:

1. **No binding to the trust provider.** A v1 record never says *which*
   provider signed it. With multiple providers possible (RFC v0.3-001), this
   is required.
2. **No binding to anti-rollback state.** The rollback record (RFC v0.3-003)
   is independent of the attestation flow. A verifier reading a v1 record
   cannot tell whether the rollback floor is current.
3. **Weak nonce semantics.** v1 nonces are optional and unstructured; v0.4's
   remote attestation transport needs a typed nonce with explicit freshness
   metadata.

The v2 schema closes these by adding three claim groups: `ProviderClaims`,
`RollbackClaims`, `KeyringClaims`. The freshness type also gains structure.

---

## 3. Goals

```text
- Single signed digest covers provider id, keyring epoch, rollback floor,
  measurement head, boot state, snapshot, health, nonce.
- Backward-compatible reader: v1 records still parse and verify.
- Forward-compatible writer: v2 records are produced by attestd whenever the
  trust-provider registry is in Enforcing phase.
- Verifier can establish that the device is "fully constrained" at the
  moment of signing without consulting external state.
- No additional kernel surface.
```

## 4. Non-Goals

```text
- No remote attestation transport. v0.4 RFC 005 introduces transport.
- No certification chain (anchor pluralism is RFC v0.3-002 scope).
- No support for attesting other devices in a fleet (v0.7/v0.8 scope).
- No structured policy claims (e.g. "the running policy bundle digest is X
  and it forbids Y"). Policy claims are deferred to v0.5 RFC 004.
```

---

## 5. External Design

### 5.1 Profile tag

```rust
#[repr(u8)]
pub enum AttestationProfile {
    FjellLocalV1Binary   = 0x01,   // legacy
    FjellLocalV1Json     = 0x02,
    FjellLocalV1Toml     = 0x03,
    FjellLocalV1PlainText= 0x04,
    /// NEW in v0.3.0: normative signed binary form with trust-provider,
    /// keyring epoch, and rollback binding.
    FjellLocalV2Binary   = 0x21,
    /// NEW: JSON projection of v2 (unsigned, advisory).
    FjellLocalV2Json     = 0x22,
}
```

### 5.2 User-visible behavior

- `attestd generate` produces a v2 record when the trust-provider registry is
  enforcing and the active provider has `SIGN_ATTESTATION` capability.
- `attestd generate` falls back to v1 when only `DevDigest32` is available.
- The semantic stream emits `AttestationRecordSigned { profile, record_id }`
  on every successful sign.
- Text-proxy summary:

  ```text
  Attestation:
    profile: FjellLocalV2Binary
    provider id: 1 (kind=Development, epoch=3)
    measurement head: sha256:abcd…
    rollback (stable): min_counter=58
    snapshot: SN000017
    health: m7-hlth status=0
    nonce: <16 hex chars>
  ```

---

## 6. Data Model

### 6.1 New claim structs

```rust
pub struct ProviderClaims {
    pub provider_id:        TrustProviderId,
    pub provider_kind:      u8,            // TrustProviderKind tag
    pub profile_tag:        u8,            // TrustProfile tag
    pub provider_generation: u16,
}

pub struct KeyringClaims {
    /// Active anchor epoch for AttestationSigning at signing time.
    pub active_epoch_attestation: u32,
    /// Active anchor epoch for ReleaseVerification at signing time.
    pub active_epoch_release:     u32,
    /// Active anchor epoch for PolicyVerification at signing time.
    pub active_epoch_policy:      u32,
    /// Sha256 over canonical encoding of the keyring snapshot (RFC v0.3-002).
    pub keyring_snapshot_digest:  Digest32,
}

pub struct RollbackClaims {
    /// Channel for which the rollback floor is reported (primary channel).
    pub channel_id:           [u8; 8],
    pub min_counter:          u64,
    pub last_advance_source:  u8,         // AdvanceSource tag
    pub trust_provider_counter_supported: bool,
    pub trust_provider_counter_value:     u64,
}

pub struct FreshnessClaimsV2 {
    pub generation:    u32,
    pub key_epoch:     u32,                // attestation signing epoch
    pub status:        u8,
    /// Nonce class — caller-declared structure for replay protection.
    pub nonce_class:   u8,
    /// 16-byte nonce. Zero-padded if caller-supplied is shorter; canonical
    /// digest covers the full 16 bytes.
    pub nonce_bytes:   [u8; 16],
}

#[repr(u8)]
pub enum NonceClass {
    LocalOnly      = 0x01,  // generated by attestd itself; not for export.
    OperatorTyped  = 0x02,  // entered through the text proxy.
    RemoteChallenge= 0x03,  // v0.4: from secure-transportd.
}
```

### 6.2 v2 record

```rust
pub struct AttestationRecordV2 {
    pub schema_version:      u16,       // = 2
    pub record_id:           AttestationRecordId,
    pub created_tick:        u64,
    pub profile:             AttestationProfile,  // = FjellLocalV2Binary
    pub provider:            ProviderClaims,
    pub keyring:             KeyringClaims,
    pub boot:                BootClaims,                 // same as v1
    pub verification:        VerificationClaims,         // same as v1
    pub measurement:         MeasurementClaims,          // same as v1
    pub snapshot:            SnapshotClaims,             // same as v1
    pub health:              HealthClaims,               // same as v1
    pub rollback:            RollbackClaims,
    pub freshness:           FreshnessClaimsV2,
    pub provenance:          Option<Provenance>,         // same as v1
}
```

### 6.3 Canonical digest

The signed digest covers a strict binary serialisation:

```text
record_digest = SHA256(
    "FJELL-ATTEST-V2" ||
    schema_version u16 LE ||
    record_id (8 B) ||
    created_tick u64 LE ||
    profile u8 ||
    provider:        provider_id u32 LE || kind u8 || profile_tag u8 || gen u16 LE ||
    keyring:         active_epoch_att u32 LE || active_epoch_rel u32 LE ||
                     active_epoch_pol u32 LE || keyring_snapshot_digest 32 B ||
    boot:            selected_slot u8 || boot_id u64 LE || kernel_digest 32 B ||
    verification:    release_digest 32 B || rootfs_digest 32 B || policy_digest 32 B ||
                     flags u8  (bit0=release_verified, bit1=rootfs_verified, bit2=policy_verified) ||
    measurement:     head_seq u64 LE || chain_digest 32 B ||
                     included_from_seq u64 LE || included_to_seq u64 LE ||
    snapshot:        snapshot_id (8 B) || snapshot_digest 32 B || reason u8 ||
    health:          target (8 B) || status u8 ||
    rollback:        channel_id (8 B) || min_counter u64 LE ||
                     last_advance_source u8 ||
                     tp_counter_supported u8 || tp_counter_value u64 LE ||
    freshness:       generation u32 LE || key_epoch u32 LE || status u8 ||
                     nonce_class u8 || nonce_bytes 16 B ||
    provenance:      present u8 || builder_tool_id 8 B || builder_version 8 B
)
```

### 6.4 Signed envelope

```rust
pub struct SignedAttestationRecordV2 {
    pub record_digest: Digest32,
    pub signature:     Signature,
    pub signed_by:     SignedByDescriptor,
}

pub struct SignedByDescriptor {
    pub provider_id:        TrustProviderId,
    pub provider_generation: u16,
    pub keyring_anchor_epoch: u32,    // anchor that holds the signing pubkey
    pub algorithm:          u8,        // SignatureAlgorithm tag
}
```

`signed_by` is **not** covered by `record_digest` — verifiers use it to pick
the verification anchor from the keyring. This is safe because the same
information appears redundantly in `provider` and `keyring` claims that *are*
covered by the digest.

---

## 7. Internal Design

### 7.1 attestd generation flow

```text
on generate(nonce, snapshot_ref):
  state = collect_state()  // pulls from measuredd, verifyd, storaged, bootctl
  v2_record = AttestationRecordV2 {
      schema_version: 2,
      record_id: next_record_id(),
      created_tick: clock_tick(),
      profile: FjellLocalV2Binary,
      provider:    build_provider_claims(active_provider_handle)?,
      keyring:     build_keyring_claims()?,
      boot:        state.boot,
      verification: state.verification,
      measurement: state.measurement,
      snapshot:    state.snapshot,
      health:      state.health,
      rollback:    build_rollback_claims(primary_channel)?,
      freshness:   FreshnessClaimsV2 {
          generation: monotonic_inc(),
          key_epoch: keyring.active_epoch(AttestationSigning),
          status: 0,
          nonce_class: nonce.class,
          nonce_bytes: nonce.bytes,
      },
      provenance:  Some(builder_provenance()),
  }
  digest = compute_v2_digest(&v2_record);
  sig = trust_provider.sign_attestation(AttestationDigest(digest))?;
  let signed = SignedAttestationRecordV2 { record_digest: digest, signature: sig,
                signed_by: build_signed_by()? };
  measuredd.append(MeasurementKind::AttestationGenerated, signed.record_digest)?;
  audit.emit(AttestationRecordSigned { record_id, profile });
  signed
```

### 7.2 attestd verification flow

```text
on verify(signed):
  if signed.algorithm == DevDigest32 && release_mode:
      return Err(DevDigestInReleaseMode);
  recompute_digest = compute_v2_digest(&signed.record);
  if recompute_digest != signed.record_digest:
      return Err(DigestMismatch);
  anchor = keyring.find(purpose=AttestationSigning,
                        epoch=signed.signed_by.keyring_anchor_epoch,
                        algorithm=signed.signed_by.algorithm)
           .ok_or(NoMatchingAnchor)?;
  if anchor.epoch < keyring.active_epoch(AttestationSigning):
      return Err(EpochBelowActive);
  sigprov.verify(anchor, signed.record_digest.bytes, signed.signature)?;
  Ok(VerifyReport { profile: V2, anchor_epoch: anchor.epoch })
```

### 7.3 v1 → v2 deprecation policy

```text
v0.3.0-alpha.1:   v2 is produced when provider in Enforcing; v1 still possible.
v0.3.0-rc.1:      v1 production is gated behind a build flag; default is v2.
v0.3.0:           v1 production is removed; v1 records still parse for reading.
v0.4.0:           v1 verification is also removed.
```

---

## 8. Security Design

### 8.1 Threat model deltas

```text
Threat T-50: Attestation record carries truthful provider id but a stale
             keyring snapshot digest (so the keyring is rotated yet the
             record looks current).
Mitigation:  keyring.keyring_snapshot_digest is covered by the canonical
             digest; verifier checks against the latest known snapshot
             through measuredd's record chain.

Threat T-51: Replay of an old v2 record against a fresh nonce.
Mitigation:  nonce_bytes (16 B) is covered by the digest; verifiers must
             supply a fresh challenge for each request.

Threat T-52: A signed-by descriptor pretends a record was signed by a
             higher-epoch anchor than it actually was.
Mitigation:  signed_by.keyring_anchor_epoch must match anchor's epoch; the
             anchor is fetched by epoch and the signature is verified against
             *that* anchor's pubkey. A wrong epoch finds no anchor → reject.

Threat T-53: Cross-profile substitution — an attacker swaps a v1 record's
             profile byte to v2.
Mitigation:  profile byte is in the canonical digest. Changing it invalidates
             the digest. Additionally, schema_version is bound.
```

### 8.2 Audit emission

```text
AttestationRecordSigned         { record_id, profile, provider_id }
AttestationVerifyFailed         { record_id, error_code }
AttestationDevDigestInRelease   { record_id }
AttestationKeyringStale         { record_id, anchor_epoch, active_epoch }
```

### 8.3 Measurement binding

Every successful sign appends a measurement event of kind
`AttestationGenerated` with `subject_digest = record_digest`. The verifier
can therefore prove that the chain head it sees actually witnessed the
attestation.

---

## 9. Memory / Resource Design

- `AttestationRecordV2` size (packed): ≈ 380 bytes; stack allocation in
  `attestd` is fine.
- `SignedAttestationRecordV2` ≈ 380 + 64 + 12 ≈ 456 bytes.
- attestd retains the last K signed records in a ring (K=8 default); total
  ≈ 3.6 KB. Suitable for static allocation.

---

## 10. Compatibility and Migration

### 10.1 Wire compatibility

- v1 parsers continue to work on v1 records (no change).
- v1 parsers reject `profile == 0x21` with `UnknownProfile`; v2 parsers
  accept both v1 (`profile in 0x01..=0x04`) and v2 (`0x21`, `0x22`).
- `fjell-tools attest verify` accepts either profile.

### 10.2 Migration plan

| Step | Action |
|------|--------|
| 1    | Add v2 types to `fjell-attestation-format` alongside v1. |
| 2    | Add canonical-digest unit tests. |
| 3    | Add `attestd` code path for v2 (gated on registry phase). |
| 4    | Add `fjell-tools attest verify` v2 path. |
| 5    | Deprecate v1 production in `v0.3.0-rc.1`. |
| 6    | Remove v1 production in `v0.3.0`. |

---

## 11. Test Strategy

### 11.1 Host unit tests

In `crates/fjell-attestation-format/src/tests.rs` (extend):

```text
- v2_digest_covers_provider_id
- v2_digest_covers_keyring_active_epochs
- v2_digest_covers_keyring_snapshot_digest
- v2_digest_covers_rollback_min_counter
- v2_digest_covers_nonce_bytes
- v2_digest_changes_when_any_field_changes      (mutation table test)
- v2_serialise_then_parse_round_trip
- v1_parser_rejects_v2_profile
- v2_parser_accepts_v1_records
- signed_by_descriptor_not_in_digest
```

Target: ≥ 12 host tests.

### 11.2 attestd integration tests (host)

```text
- attestd_generates_v2_when_enforcing
- attestd_generates_v1_when_bootstrap_and_dev_digest
- attestd_falls_back_to_v1_when_keyring_lacks_attestation_anchor
- attestd_signs_and_verifies_round_trip
```

### 11.3 QEMU negative tests

| Marker                                                | Profile |
|-------------------------------------------------------|---------|
| `NEG:ATTEST:V2_DIGEST_MISMATCH_REJECTED`             | attest  |
| `NEG:ATTEST:V2_DEV_DIGEST_IN_RELEASE_REJECTED`       | attest  |
| `NEG:ATTEST:V2_STALE_ANCHOR_EPOCH_REJECTED`          | attest  |
| `NEG:ATTEST:V2_PROVIDER_ID_MISMATCH_REJECTED`        | attest  |
| `NEG:ATTEST:V2_NONCE_REPLAY_REJECTED`                | attest  |

### 11.4 Fuzz target (v0.6 RFC 003 reservation)

```text
- fuzz: AttestationRecordV2 parser. Input: arbitrary bytes ≤ 1 KB. Property:
  parser never panics; returns Ok(record) where every accessor is reachable,
  or returns a defined error.
```

---

## 12. Acceptance Criteria

```text
- AttestationRecordV2 and SignedAttestationRecordV2 land in
  fjell-attestation-format.
- Canonical-digest formula documented under docs/src/format/.
- attestd produces v2 in QEMU enforcing-mode smoke run.
- ≥ 12 host tests + 4 host integration tests pass.
- 5 QEMU negative markers green.
- measuredd appends AttestationGenerated with v2 record_digest.
- v1 parsing remains functional for old records.
- ADR-v0.3-004 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/architecture/v0.3-004-attestation-v2.md
docs/src/development/v0.3-004-attestation-v2.md
docs/src/verification/v0.3-004-attestation-v2-invariants.md
docs/src/format/attestation-record-v2.md
docs/src/adr/v0.3-004-attestation-v2.md
```

---

## 14. Open Questions

1. **Provenance optionality** — v1 made provenance optional. v2 keeps it
   optional because not every build pipeline carries it. Should the canonical
   digest cover the *presence* bit even when absent? Yes — the table above
   already does (one byte `present u8` precedes the fields).
2. **Multi-channel rollback** — v2 carries one channel's rollback floor.
   Should the record carry all channels? Rejected: bloats the record and is
   unnecessary because v0.3.0 supports one primary channel; multi-channel
   attestation is v0.7/v0.8 fleet-scope.
3. **Nonce length** — 16 B chosen as the sweet spot between Ed25519's natural
   randomness sources and the wire-size of small QEMU records. If a future
   profile needs 32-byte nonces, a new `FjellLocalV3` profile would be added,
   not the v2 nonce field widened.

---

## 15. Release Gate (RFC-local)

```text
- Code merged.
- 12 host + 4 integration tests green.
- 5 QEMU negative markers green.
- Documentation pages exist.
- ADR-v0.3-004 Accepted.
- CHANGELOG entries filed.
```
