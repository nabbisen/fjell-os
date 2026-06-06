# RFC-v0.7.2-002: Snapshot Envelope Size Safety and Conflict-Domain Merge Rules

## Status

Draft (closes review findings **C-RB-02, C-M-01, C-M-02, C-H-08, W-H-03**)

## Target Version

`v0.7.2`

## Summary

Fix the `snapshot_digest()` buffer-overflow vulnerability identified in
crates review §5 RB-02; validate `body_len` on insertion; correct the
`ConflictDomain::Default` semantics to match the v1 absent-domain rule;
define deterministic conflict-domain merge rules; and add a
`signature_profile` field to the signed envelope so cipher-suite
agility never validates an old signature under ambiguous context.

## Motivation

The crates review identified **C-RB-02** as a release blocker: at the
declared `MAX_SNAPSHOT_RECORDS = 64` capacity, the canonical digest
buffer can overflow.

In practice this means a snapshot envelope at full capacity (within
spec) will panic the service or kernel-adjacent code path that
computes the digest.  The format-vs-implementation inconsistency must
be resolved before `Accepted` is reachable.

The conflict-domain merge rules (architect §3.4) were deferred to
v0.7.x by the handoff.  They land here.

The signing-domain question (whole-project §H-03, handoff §6.4 #1)
also resolves here.

## Goals

```text
- snapshot_digest cannot panic on any valid envelope up to spec capacity.
- SnapshotRecord::push_record rejects body_len > 64.
- ConflictDomain decode-default is ForeignAuthoritative, not the
  Default derive.
- Conflict-domain merge rules are deterministic, documented, and
  property-tested.
- SignedSnapshotEnvelope includes a signature_profile field so a
  future cipher-suite change cannot validate the old signature under
  ambiguous domain.
```

## Non-Goals

```text
- No change to the canonical content digest domain
  "FJELL-SNAPSHOT-V1" — content stability is preserved.
- No change to the underlying SHA-256 (signature_profile is the
  agility surface, not the hash itself).
```

## External Design

### Streaming digest writer

Replace the fixed-buffer formula with an incremental writer:

```rust
pub struct DigestWriter {
    hasher: sha256::Sha256,
}

impl DigestWriter {
    pub fn new() -> Self { ... }
    pub fn write_u8 (&mut self, v: u8)  { ... }
    pub fn write_u16(&mut self, v: u16) { ... }   // LE
    pub fn write_u32(&mut self, v: u32) { ... }
    pub fn write_u64(&mut self, v: u64) { ... }
    pub fn write_bytes(&mut self, b: &[u8]) { ... }
    pub fn finalize(self) -> Digest32 { ... }
}

pub fn snapshot_digest(env: &SnapshotEnvelope) -> Digest32 {
    let mut w = DigestWriter::new();
    w.write_bytes(b"FJELL-SNAPSHOT-V1");
    w.write_u16(env.schema_version);
    w.write_bytes(&env.source_identity_digest.0);
    // ... etc ...
    w.finalize()
}
```

No fixed-size stack buffer.  No size cap on input.

### `SnapshotRecord` validation

```rust
impl SnapshotEnvelope {
    pub fn push_record(&mut self, r: SnapshotRecord) -> Result<(), SnapshotError> {
        if r.body_len as usize > SNAPSHOT_RECORD_BODY_MAX {
            return Err(SnapshotError::BodyTooLarge);
        }
        if self.record_count as usize >= MAX_SNAPSHOT_RECORDS {
            return Err(SnapshotError::CapacityExhausted);
        }
        // ...
    }
}
```

`SNAPSHOT_RECORD_BODY_MAX` is exposed; today's slot size is 64.

### `ConflictDomain` default fix

Remove `derive(Default)` from `ConflictDomain` (or override it).
Document explicitly that absent-domain in v1 decode resolves to
`ForeignAuthoritative`:

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum ConflictDomain {
    LocallyConfirmed     = 0x01,
    ForeignAuthoritative = 0x02,
    Pending              = 0x03,
    Contested            = 0x04,
}

impl ConflictDomain {
    /// v1 absent-domain decode rule per ADR-v0.7-004.
    pub const V1_DEFAULT: Self = Self::ForeignAuthoritative;
}
```

The decoder uses `V1_DEFAULT` explicitly when reading a v1 envelope.

### Conflict-domain merge rules

When a record arrives in an incoming snapshot, the local store may
already have a record with the same `(kind, seq)`.  Rules:

| Incoming domain | Local state | Action |
|-----------------|-------------|--------|
| `LocallyConfirmed` | absent | append as `LocallyConfirmed` |
| `LocallyConfirmed` | local `LocallyConfirmed` | keep local (no-op) |
| `LocallyConfirmed` | local `Pending` | reject `IncomingClaimsLocalConfirm` |
| `ForeignAuthoritative` | absent | append as `ForeignAuthoritative` |
| `ForeignAuthoritative` | local `LocallyConfirmed` | mark local `Contested` |
| `ForeignAuthoritative` | local `ForeignAuthoritative` | keep local (no-op) |
| `Pending` | absent | append as `Pending` |
| `Pending` | local any | keep local (incoming is hint) |
| `Contested` | any | reject `ContestedNotImportable` |

Rules are deterministic by `(incoming_domain, local_state)`.  No
clock/timestamp dependency.

### Signature profile

```rust
pub struct SignedSnapshotEnvelope {
    pub envelope:          SnapshotEnvelope,
    pub signature_profile: SignatureProfile,
    pub signature:         AttestationSignature,
}

#[repr(u8)]
pub enum SignatureProfile {
    Ed25519Sha256 = 0x01,    // current default; FJELL-SNAPSHOT-SIGN-V1 domain
    // 0x02..0xFF reserved
}
```

The signing-side domain string becomes:

```text
"FJELL-SNAPSHOT-SIGN-V1" || signature_profile_u8 || snapshot_digest
```

A v0.8 cipher-suite addition allocates a new `SignatureProfile`
discriminant and continues to validate; an envelope signed under one
profile can never accidentally validate as another.

## Data Model

### `SnapshotError` (new)

```rust
#[repr(u8)]
pub enum SnapshotError {
    BodyTooLarge        = 0x01,
    CapacityExhausted   = 0x02,
    UnknownSchema       = 0x03,
    UnknownDomain       = 0x04,
}
```

`SnapshotEnvelope::push_record` now returns `Result<(), SnapshotError>`
instead of `Result<(), ()>`.

## Internal Design

### Frozen schema update

`crates/fjell-snapshot-format/schema/snapshot-v2.frozen` adds:

```text
sig signature_profile u8
sig signature_bytes   u8 [64]
```

This is *not* a BREAKING-SCHEMA: the content digest is unchanged.  The
signed envelope wrapper is a new logical layer.

### Property tests added

In `fjell-store-model` (or a new `fjell-sync-model` if the dependency
graph gets noisy):

```text
SS1 merge_rule_deterministic
SS2 merge_locally_confirmed_keeps_local
SS3 merge_foreign_over_local_confirmed_marks_contested
SS4 merge_pending_never_overwrites_authoritative
SS5 contested_inbound_always_rejected
SS6 idempotent_replay_same_outcome
```

1000 cases per property.

### Acceptance tests

Per crates review §13:

```text
SNAPSHOT:DIGEST_FULL_CAPACITY_NO_PANIC
SNAPSHOT:BODY_LEN_OVER_64_REJECTED
SNAPSHOT:V1_MISSING_DOMAIN_FOREIGN_AUTHORITATIVE
```

These become host unit tests in `fjell-snapshot-format`.

## Security Design

### Buffer overflow

Streaming digest writer has no fixed buffer.  The crates-review
RB-02 panic case is eliminated by construction.

### Signing-domain agility

Adding `signature_profile` to the domain prevents a hypothetical
future where:

```text
- v0.8 introduces SignatureProfile::Ed25519Sha512.
- An attacker captures a v0.7 Ed25519Sha256 signature.
- Replays it claiming to be Sha512.
- Verifier checks against the wrong profile and accepts.
```

The profile byte in the signing domain means the v0.7 signature can
NEVER validate under a future profile, regardless of how the verifier
selects the key.

### Conflict-domain rule fail-closed

The merge table has explicit reject cases.  No default-allow.

## Memory / Resource Design

`DigestWriter` adds approximately one SHA-256 context (~104 B) to the
stack of the snapshot path.  This replaces a 4 KiB buffer, so net
memory pressure decreases.

## Compatibility and Migration

- `SnapshotEnvelope::push_record` return type changes from
  `Result<(), ()>` to `Result<(), SnapshotError>`.  Callers using
  `?` are unaffected; callers matching `Err(())` must update.
- `ConflictDomain::default()` no longer compiles if the `Default`
  derive was relied on.  Replace with `ConflictDomain::V1_DEFAULT`.
- v1 envelopes continue to decode as before (the absent-domain rule
  is unchanged in semantics; the implementation just references
  `V1_DEFAULT` explicitly).

This is **not** a BREAKING-SCHEMA bump because the on-the-wire content
digest is identical.  The signed wrapper is a new optional layer that
v0.7.2 services emit but v0.7.1 services would silently ignore (they
do not yet verify signatures).

## Test Strategy

```text
- fjell-snapshot-format unit tests:
    - snapshot_digest with MAX_SNAPSHOT_RECORDS and body_len = 64
      never panics
    - push_record(body_len=65) → BodyTooLarge
    - push_record at capacity+1 → CapacityExhausted
    - v1 envelope decoded with absent-domain shows
      ForeignAuthoritative
- 6 merge-rule property tests × 1000 cases each
- Signature-profile round trip: sign profile X, verify profile X OK,
  verify profile Y FAIL
```

## Acceptance Criteria

```text
- snapshot_digest passes a fuzz test at full capacity (no panic).
- All three SNAPSHOT:* acceptance tests pass.
- 6 merge-rule property tests green.
- SnapshotImportOutcome::Accepted is reachable when (and only when)
  the merge table allows it.
- ADR-v0.7.2-002 filed (the merge rules are an ADR-worthy decision).
```

## Documentation Requirements

```text
- docs/src/reference/snapshot-merge-rules.md — the full merge table.
- docs/src/reference/snapshot-format.md updated for SignatureProfile.
- ADR-v0.7-004 amended with a forward reference to ADR-v0.7.2-002
  for merge semantics.
- BREAKING-SCHEMA: NOT used (intentional; content digest unchanged).
```

## Open Questions

```text
1. Should Pending records ever cross a sync boundary, or only stay
   local? Proposal: Pending may cross (operators may want hint
   propagation), but never overwrites authoritative state.

2. What happens to a record promoted from Pending to LocallyConfirmed
   after a Foreign claim? Proposal: the existing rule applies — the
   incoming Foreign sees a LocallyConfirmed and yields a Contested
   marker locally; operator intervention required.

3. Do we need a per-kind merge override? Proposal: not in v0.7.2;
   revisit in v0.8 if fleet ops requires per-kind policy.
```

## Release Gate

`TEST:V0.7-SYNC:PASS` is extended to require:

```text
- "syncd: digest_full_capacity_no_panic"
- "syncd: merge_rule_property_tests=6/6"
- "syncd: signature_profile_separator_verified"
```
