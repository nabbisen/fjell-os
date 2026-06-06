# RFC-v0.3-003: Anti-Rollback Metadata and upgraded Local Confirmation Hardening

**Status.** Implemented (v0.3.0)

## Status

Draft (revised, supersedes pack v0.3-003 draft)

## Target Version

`v0.3.0`.

## Phase

Hardware Trust Abstraction — Epic C (Anti-Rollback and upgraded Hardening).

## Related Work

- v0.3 RFC 001 — `HardwareTrustProvider::read_anti_rollback_counter`.
- v0.3 RFC 002 — `KeyEpoch` is also bound into release metadata here.
- v0.2 RFCs 042/043 (release gate); v0.2 RFC 057 (bootctl service extraction);
  v0.2 RFC 058 (service-manager ready tracking).
- v0.4 RFC 004 — staged-upgraded integration (consumes this RFC's
  `ReleaseMetadata`).

---

## 1. Summary

Define `ReleaseMetadata`, the per-release record that binds a release manifest
digest to:

- a **release counter** that is monotonic across all releases ever installed;
- the `KeyEpoch` of the anchor that signed the release;
- the `MeasurementHead` at the time of staging;
- the `TrustProviderId` that authorised the staging;
- a `MinimumRequiredCounter` enforced by `upgraded` and by the boot path.

Harden `upgraded` so that local A/B confirmation refuses to confirm a slot
whose `ReleaseMetadata.counter` is below the persisted `min_counter`, even if
the signature would otherwise verify. Persist `min_counter` through `storaged`
as an append-only record.

---

## 2. Motivation

v0.2's A/B upgrade is signature-verified and confirms a candidate slot after a
positive health probe. But the boundary has one gap:

- **Replay of a signed-but-old release.** If an attacker can deliver a
  previously-signed release bundle (e.g., a known-vulnerable version
  rev *N-1*), v0.2 has nothing that says "we already ran *N*, refuse *N-1*."
  The signature math succeeds because the bundle is genuinely signed by the
  release anchor.

Anti-rollback metadata closes this. The release author embeds a monotonic
counter; the OS persists the highest counter ever confirmed; `upgraded`
refuses candidates with a lower counter.

The mechanism is local in v0.3.0 (no remote anti-rollback service); v0.4
extends it across the secure-transport channel for fleet operations.

---

## 3. Goals

```text
- Each release manifest carries a release_counter (u64) and an
  embedded_min_counter (u64) that the release author chose at sign time.
- upgraded enforces: candidate.release_counter >= persisted_min_counter.
- After confirmation, persisted_min_counter is updated to
  max(persisted_min_counter, candidate.release_counter).
- Boot-time bootctl refuses to mark a slot bootable if its metadata fails the
  check, even if it was previously confirmed (defence in depth against
  bootctl-only writes).
- Anti-rollback failures produce a defined audit event and a defined semantic
  intent in the text proxy.
- A NullTrustProvider boot rejects all upgrades (counter cannot be read).
```

## 4. Non-Goals

```text
- No clock-based expiry. Counters are sequence numbers, not timestamps.
- No remote anti-rollback (v0.4 RFC 004).
- No automatic counter advance on dev builds. The release-builder tool will
  bump the counter; CI may pin it.
- No support for downgrades through an "override" flag. A genuine downgrade
  requires a recovery flow that erases the slot and treats the new install as
  a fresh provision; that path is recovery scope (recoveryd).
```

---

## 5. External Design

### 5.1 User-visible behavior

- Attempting to install a release whose counter is below the persisted
  minimum results in:

  ```text
  upgraded: rejecting release counter=42 below persisted minimum=58
  semantic intent: NEG:UPGRADE:ROLLBACK_REJECTED
  ```

- A successful confirmation advances `min_counter`:

  ```text
  upgraded: confirmed slot=B counter=63 (was 58); persisted min_counter=63
  ```

- The text proxy shows:

  ```text
  Update state:
    active slot:        A (counter=58)
    candidate:          none
    min_counter:        58
    next allowed:       any counter >= 58
  ```

### 5.2 Counter semantics

The release counter is **not** a version number. It is a per-channel,
strictly monotonic sequence number assigned by the release-builder. Two
unrelated channels (e.g., "stable" and "lts") may use independent counters,
but they are tracked separately by `storaged` (one persisted `min_counter`
per `release_channel_id`).

### 5.3 Per-channel scoping

`ReleaseMetadata.channel_id` is an 8-byte ASCII channel identifier. The
persisted `min_counter` is keyed by `channel_id`. Cross-channel installs are
neither blocked nor merged at this layer; that policy lives in `verifyd`
(future v0.3.x RFC).

---

## 6. Data Model

### 6.1 Canonical release metadata

```rust
pub const RELEASE_METADATA_VERSION: u16 = 1;

pub struct ReleaseMetadata {
    pub schema_version:        u16,           // = RELEASE_METADATA_VERSION
    pub channel_id:            [u8; 8],       // ASCII, zero-padded
    pub release_counter:       u64,           // monotonic within channel
    pub embedded_min_counter:  u64,           // author-side rollback floor
    pub release_manifest_digest: Digest32,    // sha256 of the manifest itself
    pub signing_anchor_epoch:  KeyEpoch,      // from RFC v0.3-002
    pub trust_provider_id:     TrustProviderId, // staging provider
    pub measurement_at_stage:  Digest32,      // MeasurementHead.chain_digest
    pub created_tick:          u64,           // local kernel tick at stage time
    pub provenance:            Provenance,    // see 6.2
    pub metadata_digest:       Digest32,      // sha256 over all fields above
                                              // with metadata_digest=0
}

pub struct Provenance {
    pub builder_tool_id: [u8; 8],   // e.g. b"fjell-bu"
    pub builder_version: [u8; 8],   // e.g. b"0.3.0a1\0"
}
```

`metadata_digest` is what the release signature actually covers (in the v0.3
flow). Bundling the digest into the metadata itself prevents the signature
from being lifted off one metadata blob and pasted onto a forged one.

### 6.2 Persistent rollback record

```rust
pub struct RollbackRecord {
    pub schema_version:   u16,           // = 1
    pub channel_id:       [u8; 8],
    pub min_counter:      u64,
    pub last_advance_tick: u64,
    pub last_advance_source: AdvanceSource,
    pub record_digest:    Digest32,
}

#[repr(u8)]
pub enum AdvanceSource {
    UpgradedConfirmation = 0x01,
    RecoveryReset        = 0x02,
    BootctlPromotion     = 0x03,
}
```

`RollbackRecord` is appended to `storaged`'s log; the *latest* record per
`channel_id` is authoritative.

---

## 7. Internal Design

### 7.1 upgraded confirmation flow (revised)

```text
on confirm_candidate(slot):
  release_meta = read_metadata_from_slot(slot)?;
  verify_signature(release_meta.metadata_digest, signature, keyring)?;

  persisted = storaged.latest_rollback_record(release_meta.channel_id)?;
  if release_meta.release_counter < persisted.min_counter:
      emit_audit(UpgradeRollbackRejected { ... });
      return Err(Rollback);

  // Also enforce the embedded floor (author can lift the floor unilaterally;
  // we honour it as long as it doesn't *lower* the persisted minimum).
  if release_meta.embedded_min_counter > release_meta.release_counter:
      return Err(MetadataInconsistent);

  rollback_counter = trust_provider.read_anti_rollback_counter()?;
  // Soft binding in v0.3.0; full binding requires production provider:
  emit_audit(UpgradeRollbackCounterRead {
      counter: rollback_counter,
      meta_counter: release_meta.release_counter });

  bootctl.confirm_slot(slot)?;
  storaged.append_rollback_record(RollbackRecord {
      channel_id: release_meta.channel_id,
      min_counter: max(persisted.min_counter, release_meta.release_counter),
      last_advance_tick: now_tick,
      last_advance_source: AdvanceSource::UpgradedConfirmation,
      ...
  })?;
```

### 7.2 bootctl integration

bootctl reads `ReleaseMetadata` from the slot header during boot. If the
counter is below the persisted minimum for the channel, bootctl refuses to
mark the slot bootable and falls through to the other slot (or to recovery if
neither slot validates). This is defence in depth in case upgraded is
compromised or replaced.

### 7.3 storaged record kind

```text
StoreRecordKind::RollbackRecord  = 0x14   // (new in v0.3)
```

Recovery scan: while reading the append-only log, accumulate the highest
`(channel_id → min_counter)` mapping. The result is the authoritative state at
boot.

### 7.4 Failure of trust-provider counter read

If `trust_provider.read_anti_rollback_counter()` returns
`TrustError::NotSupported` (e.g., the provider does not expose a counter),
v0.3.0 treats this as a soft success — the persisted-counter check still
applies. A future hardware profile RFC may upgrade this to a hard requirement.

---

## 8. Security Design

### 8.1 Threat model deltas

```text
Threat T-40: Replay of a signed-but-older release bundle.
Mitigation:  release_counter check; signature verifies but counter check
             rejects.

Threat T-41: Adversary modifies persisted_min_counter via direct storage
             write (e.g., a compromised storaged).
Mitigation:  RollbackRecord.record_digest covers the counter; record sequence
             is append-only and the highest seen is authoritative;
             bootctl re-reads ReleaseMetadata and refuses to boot a slot
             whose embedded counter is below the metadata-claimed value.

Threat T-42: Adversary picks an old release and bumps the metadata counter
             without re-signing.
Mitigation:  metadata_digest covers release_counter; signature covers
             metadata_digest. Re-bumping the counter invalidates the digest
             and therefore the signature.

Threat T-43: Channel-id confusion (release for channel "lts" presented as
             channel "stable").
Mitigation:  metadata_digest covers channel_id; bootctl compares against the
             slot's recorded channel_id during promotion.

Threat T-44: Recovery flow used to "wash out" the min_counter and downgrade.
Mitigation:  RecoveryReset is a defined AdvanceSource that *retains* the
             min_counter rather than zeroing it. Genuine reprovisioning uses
             the v0.4 fleet-managed reset which is signed.
```

### 8.2 Audit emission

```text
UpgradeRollbackRecordPersisted   { channel_id, min_counter, source }
UpgradeRollbackRejected          { channel_id, attempted, min_counter }
UpgradeMetadataInconsistent      { channel_id, embedded, counter }
UpgradeProviderCounterMissing    { provider_id }   // soft warning
BootRollbackBlockedSlot          { slot, embedded_counter, min_counter }
```

### 8.3 Semantic state events

```text
intent: UPDATE.ROLLBACK_BLOCKED
fields: channel_id, attempted_counter, min_counter, signing_anchor_epoch
text proxy: "Update rejected: counter 42 below required minimum 58."
```

---

## 9. Memory / Resource Design

- `ReleaseMetadata` packed size: 2+8+8+8+32+4+4+32+8+16+32 = 154 bytes; stack
  allocation fine.
- `RollbackRecord` packed size: 2+8+8+8+1+32 = 59 bytes.
- storaged sees a small constant number of these records over the lifetime of
  the device; pruning is unnecessary.

---

## 10. Compatibility and Migration

### 10.1 Compatibility with v0.2 upgrade format

v0.2 release bundles do not carry `ReleaseMetadata.release_counter`. The
migration policy:

- A bundle whose manifest predates `RELEASE_METADATA_VERSION=1` is treated as
  `release_counter = 0`, `channel_id = b"legacy--"`, and is rejected by
  `upgraded` if any non-legacy record exists in storaged.
- On a fresh install (no persisted rollback record), v0.2 bundles install
  successfully and create the legacy record with `min_counter = 0`.
- A subsequent v0.3-format bundle is accepted only if it advertises the
  legacy channel or a fresh channel; cross-channel migration requires a
  signed channel-switch metadata record (deferred to v0.4 RFC 004).

### 10.2 Migration plan

| Step | Action |
|------|--------|
| 1    | Land `ReleaseMetadata` and `RollbackRecord` types in `fjell-upgrade-format`. |
| 2    | Extend `verifyd` to verify metadata signature (uses RFC v0.3-002). |
| 3    | Wire `upgraded` to consult `storaged` for the rollback record. |
| 4    | Add bootctl check that re-reads slot metadata. |
| 5    | Add the QEMU negative test that delivers a stale bundle. |
| 6    | Update the release-builder tool to write `ReleaseMetadata`. |

---

## 11. Test Strategy

### 11.1 Host unit tests

In `crates/fjell-upgrade-format/src/tests.rs` (extend existing):

```text
- release_metadata_digest_covers_counter
- release_metadata_digest_covers_channel_id
- release_metadata_digest_covers_anchor_epoch
- release_metadata_serialise_then_parse_round_trip
- release_metadata_bad_digest_rejected
- release_metadata_inconsistent_embedded_min_rejected
- rollback_record_serialise_then_parse_round_trip
- rollback_record_bad_digest_rejected
```

### 11.2 storaged tests

In `crates/fjell-storaged` or a new test bin:

```text
- rollback_record_replay_from_log_takes_highest
- rollback_record_per_channel_isolation
- rollback_record_corrupt_record_skipped
```

### 11.3 QEMU negative tests

| Marker                                            | Profile  |
|---------------------------------------------------|----------|
| `NEG:UPGRADE:ROLLBACK_REJECTED`                  | upgrade  |
| `NEG:UPGRADE:METADATA_DIGEST_MISMATCH_REJECTED`  | upgrade  |
| `NEG:UPGRADE:LEGACY_AFTER_V03_REJECTED`          | upgrade  |
| `NEG:BOOT:ROLLBACK_BLOCKS_SLOT`                  | upgrade  |
| `NEG:UPGRADE:CHANNEL_MISMATCH_REJECTED`          | upgrade  |

### 11.4 Property test (deferred to v0.6 RFC 001)

```text
- "for any sequence of installs and confirmations, persisted min_counter is
   monotone non-decreasing per channel"
```

---

## 12. Acceptance Criteria

```text
- ReleaseMetadata and RollbackRecord live in fjell-upgrade-format with host
  tests passing.
- storaged accepts and replays RollbackRecord.
- upgraded rejects bundles below persisted min_counter.
- bootctl re-checks slot metadata at boot.
- All 5 QEMU negative markers green.
- Legacy v0.2 bundles continue to install on a fresh device (smoke test).
- ADR-v0.3-003 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/architecture/v0.3-003-anti-rollback.md
docs/src/development/v0.3-003-anti-rollback.md
docs/src/verification/v0.3-003-anti-rollback-invariants.md
docs/src/adr/v0.3-003-anti-rollback.md
docs/src/format/release-metadata.md
docs/src/format/rollback-record.md
```

---

## 14. Open Questions

1. **embedded_min_counter ergonomics** — is it useful for the release author
   to set this independently of the counter itself? Resolution: yes, because
   it lets a release author declare "do not install older than X" in a single
   build without coordinating with the fleet. Keep the field.
2. **Channel switch** — how does an operator move a device from channel
   "stable" to channel "lts"? Out of scope for v0.3.0; v0.4 RFC 004 will add a
   signed `ChannelSwitchRecord` that resets `min_counter` for the new channel
   while retaining the old.
3. **Recovery and counter floor** — `RecoveryReset` retains min_counter by
   design. Is there ever a legitimate need to lower it? Resolution: no,
   except via vendor-signed factory reset which is out of scope until v0.4.

---

## 15. Release Gate (RFC-local)

```text
- Format crates extended, host tests green.
- upgraded + storaged + bootctl integrated.
- 5 QEMU markers green.
- ADR-v0.3-003 Accepted.
- CHANGELOG entry filed.
```
