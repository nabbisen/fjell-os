# RFC-v0.7.2-003: NodeIdentity Constructor Safety and Trust Mode Fail-Closed Semantics

## Status

Draft (closes review findings **C-H-02, C-H-03, C-H-04, W-H-04,
C-M-05, C-M-10**)

## Target Version

`v0.7.2`

## Summary

Make `NodeIdentity` construction always produce a valid digest;
prevent `NodeIdentityPolicy::permits()` from panicking on malformed
`allowed_count`; make `TrustMode::Fleet` and `TrustMode::Open`
fail-closed by default; expose UTF-8 errors via `try_as_str`; and
land the persistent replay-nonce cache that the snapshot import
pipeline depends on.

## Motivation

The crates review identified four identity-shape concerns and the
whole-project review noted the trust-mode fail-closed requirement:

- **C-H-02**: `NodeIdentityPolicy::permits()` slices
  `allowed_profiles[..allowed_count]` with no bound check.  If
  `allowed_count > 4` (the array length) the slice panics.
- **C-H-03**: `TrustMode::Fleet` only checks that
  `pinned_roster.is_some()` — no roster lookup, signature, membership,
  generation, or revocation.
- **C-H-04**: `NodeIdentity::new()` produces a zero
  `identity_digest` and relies on the caller to compute and assign it.
  Easy to forget; identityd's stub did exactly that.
- **W-H-04**: `TrustMode::Open` accepts any node by design; `Fleet`
  without a roster accepts every node. Both must be gated behind
  explicit feature/profile markers.
- **C-M-05**: `NodeAlias::as_str()` returns `""` on invalid UTF-8 via
  `unwrap_or("")`.  Hides malformed/malicious aliases.
- **C-M-10**: `syncd` uses `SnapshotEnvelope::new_v2()` but no replay
  cache exists.

## Goals

```text
- NodeIdentity is constructible only via safe constructors that
  produce a non-zero identity_digest.
- NodeIdentityPolicy is validated at construction; permits() never
  panics.
- TrustMode::Fleet fails closed unless a roster validation provider
  is registered.
- TrustMode::Open requires an explicit insecure-profile marker at
  build/runtime.
- NodeAlias exposes try_as_str() returning Result.
- Persistent replay-nonce cache exists and is consulted by syncd
  before accepting a snapshot.
```

## Non-Goals

```text
- Full roster signature/generation pipeline (v0.8 fleet work).
- Multi-tenant roster (single roster per node in v0.7.2).
- Distributed replay cache (single-node cache is sufficient until
  v0.8 fleet sync).
```

## External Design

### Safe constructors for NodeIdentity

```rust
impl NodeIdentity {
    /// Construct a NodeIdentity and compute its digest in one step.
    /// This is the ONLY public path to a valid NodeIdentity.
    pub fn build(b: NodeIdentityBuilder) -> Result<Self, IdentityError> {
        let mut n = Self {
            schema_version:     NODE_IDENTITY_SCHEMA_VERSION,
            node_id:            b.node_id,
            alias:              b.alias,
            created_tick:       b.created_tick,
            trust_provider_id:  b.trust_provider_id,
            trust_profile_tag:  b.trust_profile_tag,
            attestation_pubkey: b.attestation_pubkey,
            platform_digest:    b.platform_digest,
            board_digest:       b.board_digest,
            identity_digest:    Digest32::default(),
        };
        n.identity_digest = identity_digest(&n);
        if n.identity_digest == Digest32::default() {
            return Err(IdentityError::DigestComputationFailed);
        }
        Ok(n)
    }

    /// Validate that the stored identity_digest matches a freshly
    /// computed one. Returns Err on mismatch.
    pub fn validate_digest(&self) -> Result<(), IdentityError> { ... }
}
```

The old `NodeIdentity::new()` is removed (or kept `pub(crate)` with a
`/// internal use only` comment).

### `NodeIdentityPolicy::validate` and safe `permits`

```rust
impl NodeIdentityPolicy {
    pub fn validate(&self) -> Result<(), PolicyError> {
        if self.allowed_count as usize > self.allowed_profiles.len() {
            return Err(PolicyError::AllowedCountOverflow);
        }
        if matches!(self.mode, TrustMode::Fleet) && self.pinned_roster.is_none() {
            return Err(PolicyError::FleetWithoutRoster);
        }
        Ok(())
    }

    /// permits() now returns Decision and never panics.
    pub fn permits(&self, profile_tag: u8) -> Decision {
        // Validate first; if invalid, deny.
        if self.validate().is_err() { return Decision::Deny; }
        match self.mode {
            TrustMode::Open  => Decision::AllowInsecure,    // requires explicit marker
            TrustMode::Fleet => Decision::NeedsRosterValidation(self.pinned_roster.unwrap()),
            TrustMode::SameFamily => {
                let n = self.allowed_count as usize;
                if n == 0 { return Decision::Allow; }
                if self.allowed_profiles[..n].contains(&profile_tag) {
                    Decision::Allow
                } else {
                    Decision::Deny
                }
            }
        }
    }
}

pub enum Decision {
    Allow,
    AllowInsecure,                  // TrustMode::Open
    NeedsRosterValidation(RosterRef),
    Deny,
}
```

### Insecure-profile marker

`TrustMode::Open` returning `Decision::AllowInsecure` is then handled
by the caller:

```rust
match policy.permits(peer.profile_tag) {
    Decision::Allow => proceed(),
    Decision::AllowInsecure => {
        #[cfg(feature = "trust-mode-open")]
        { proceed_with_audit() }
        #[cfg(not(feature = "trust-mode-open"))]
        { reject(SnapshotImportError::IdentityNotPermitted) }
    },
    Decision::NeedsRosterValidation(r) => match roster_provider {
        Some(p) => match p.verify(r, peer) {
            Ok(_)  => proceed(),
            Err(e) => reject(SnapshotImportError::IdentityNotPermitted),
        },
        None => reject(SnapshotImportError::IdentityNotPermitted),
    },
    Decision::Deny => reject(SnapshotImportError::IdentityNotPermitted),
}
```

A v0.7.2 build with the `trust-mode-open` feature disabled CANNOT
accept `TrustMode::Open`.  This is the fail-closed requirement.

### `NodeAlias::try_as_str`

```rust
impl NodeAlias {
    /// Strict UTF-8 access. Returns Err if invalid.
    pub fn try_as_str(&self) -> Result<&str, core::str::Utf8Error> {
        let end = self.0.iter().position(|&b| b == 0).unwrap_or(32);
        core::str::from_utf8(&self.0[..end])
    }

    /// Lossy UTF-8 access for display only. Replaces invalid sequences.
    /// Use only in human-facing diagnostic output.
    pub fn as_str_lossy(&self) -> &str {
        self.try_as_str().unwrap_or("<invalid utf-8 alias>")
    }
}
```

`as_str()` (the old name) is removed.  Callers explicitly choose
between strict and lossy access.

### Replay-nonce cache

A new module `fjell-syncd::replay_cache`:

```rust
pub struct ReplayCache {
    entries: [Option<NonceEntry>; REPLAY_CACHE_CAPACITY],
    cursor:  usize,
}

pub struct NonceEntry {
    pub source_identity: Digest32,
    pub nonce:           [u8; 16],
    pub seen_tick:       u64,
}

impl ReplayCache {
    /// Returns Err(ReplayDetected) if (source_identity, nonce) is
    /// already known.  Otherwise inserts and returns Ok.
    pub fn check_and_insert(
        &mut self,
        source_identity: Digest32,
        nonce: [u8; 16],
        now_tick: u64,
    ) -> Result<(), ReplayError> { ... }
}
```

Capacity: `REPLAY_CACHE_CAPACITY = 256` (configurable; bounded to fit
in syncd memory budget).  Older entries evict LRU.  Persisted to
storaged on every insert (kind `0x0040`).

## Data Model

### `IdentityError`

```rust
#[repr(u8)]
pub enum IdentityError {
    DigestComputationFailed = 0x01,
    DigestMismatch          = 0x02,
    InvalidAlias            = 0x03,
}
```

### `PolicyError`

```rust
#[repr(u8)]
pub enum PolicyError {
    AllowedCountOverflow = 0x01,
    FleetWithoutRoster   = 0x02,
}
```

### `ReplayError`

```rust
#[repr(u8)]
pub enum ReplayError {
    ReplayDetected   = 0x01,
    CacheUnavailable = 0x02,
}
```

## Internal Design

### Builder ergonomics

```rust
pub struct NodeIdentityBuilder {
    pub node_id:            NodeId,
    pub alias:              NodeAlias,
    pub created_tick:       u64,
    pub trust_provider_id:  u32,
    pub trust_profile_tag:  u8,
    pub attestation_pubkey: AttestationPubkey,
    pub platform_digest:    Digest32,
    pub board_digest:       Digest32,
}
```

Callers fill the builder, then call `NodeIdentity::build(b)`.

### identityd integration

identityd (RFC-v0.7.2-001) uses `NodeIdentity::build` and refuses to
ready if `build` returns `Err`.  Existing zero-digest stub paths are
deleted.

### Replay cache integration in syncd

```text
syncd::import_envelope():
  1. Parse envelope, find source_identity, nonce.
  2. ReplayCache::check_and_insert(...).
  3. If ReplayDetected → reject SnapshotImportError::ReplayDetected.
  4. Proceed to signature/identity policy/merge.
```

## Security Design

### Defence-in-depth at identity boundary

| Threat | Mitigation |
|--------|------------|
| Zero-digest identity persisted | `NodeIdentity::build` always computes digest; `build` returns Err on zero result. |
| Panic on malformed policy | `validate()` runs at construction; `permits()` runs validate internally and returns Deny. |
| Fleet mode without roster | `validate()` fails; `permits()` returns Deny. |
| Open mode in production | feature-gated; without the flag, `AllowInsecure` is rejected. |
| Replayed snapshot envelope | `ReplayCache` rejects with ReplayDetected. |
| Crafted UTF-8 alias bypassing log filters | `try_as_str()` returns Err; only `as_str_lossy()` (display-only) is lossy. |

### Fail-closed audit

Any `Decision::Deny`, `IdentityError`, `PolicyError`, or
`ReplayError` emits:

```text
AUDIT_IDENTITY_REJECTED      = 0x0205
AUDIT_POLICY_INVALID         = 0x0206
AUDIT_REPLAY_DETECTED        = 0x0207
```

Audit events are pinned-critical (not subject to rate limiting per
RFC v0.5-005).

## Memory / Resource Design

- `ReplayCache`: 256 entries × ~56 B = ~14 KiB per syncd task.
- Persisted cache: appended to storaged on insert; reload on syncd
  start.
- `NodeIdentityBuilder` is constructed on the stack; no heap.

## Compatibility and Migration

- `NodeIdentity::new()` is removed (or `pub(crate)`).  This is a
  **breaking API change** for any out-of-tree consumer.  Internal
  callers (identityd) are updated as part of this RFC.
- `permits()` return type changes from `bool` to `Decision`.  Callers
  must match on the new enum.  identityd and syncd callers are
  updated.
- `NodeAlias::as_str` removed.  Replace with `try_as_str` or
  `as_str_lossy` explicitly.
- The wire format of `NodeIdentity` is unchanged (still 192 B canonical).

## Test Strategy

```text
- identity_policy_allowed_count_over_capacity_rejected
- identity_policy_permits_never_panics_on_malformed_count
- identity_policy_fleet_without_roster_denies
- identity_policy_open_without_feature_denies
- identity_build_produces_nonzero_digest
- identity_build_with_zero_inputs_still_produces_nonzero
- identity_alias_invalid_utf8_returns_err
- replay_cache_repeat_nonce_rejected
- replay_cache_lru_eviction_works
- replay_cache_survives_syncd_restart
```

10 host tests + 4 property cases each for the boundary-checking ones.

## Acceptance Criteria

```text
- IDENTITY:NEW_WITH_DIGEST_NONZERO test passes.
- IDENTITY:ZERO_DIGEST_REJECTED_ON_LOAD test passes.
- IDENTITY:ALLOWED_COUNT_OVERFLOW_REJECTED test passes.
- IDENTITY:FLEET_MODE_REQUIRES_VALID_ROSTER test passes.
- SYNC:REPLAY_NONCE_REJECTED test passes.
- SYNC:FLEET_MODE_WITHOUT_ROSTER_VALIDATION_REJECTED test passes.
- Default build features do NOT include trust-mode-open.
- ADR-v0.7.2-003 filed.
```

## Documentation Requirements

```text
- docs/src/reference/identity-construction.md — the build pattern.
- docs/src/reference/trust-modes.md — fail-closed semantics.
- UNSAFE_CHARTER.md updated if the build path triggers new unsafe.
```

## Open Questions

```text
1. Should NodeIdentityBuilder enforce field-by-field validation
   (e.g., reject all-zero pubkey)? Proposal: yes for pubkey; alias
   may be all-zero (anonymous node).

2. Replay-cache capacity — 256 entries OK? Larger? Proposal: 256 is
   sized for v0.7.2 single-peer sync; v0.8 fleet may need 4096.

3. Lossy alias display vs strict — where is lossy used? Proposal:
   only in proxy-text intent rendering; everywhere else strict.
```

## Release Gate

`TEST:V0.7-SYNC:PASS` is extended:

```text
- "identityd: build_with_nonzero_digest"
- "syncd: replay_cache=ready capacity=256"
- "syncd: trust_mode_open=disabled" (in default build)
```
