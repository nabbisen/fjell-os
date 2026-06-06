# RFC-v0.3-002: Keyring, Key Purpose, Signature Provider, and Key Epoch Model

**Status.** Implemented (v0.3.0)

## Status

Draft (revised, supersedes pack v0.3-002 draft)

## Target Version

`v0.3.0` (lands directly on top of RFC v0.3-001).

## Phase

Hardware Trust Abstraction — Epic B (Keyring and Signature Foundation).

## Related Work

- v0.3 RFC 001 — defines `TrustProviderId`, `KeyPurpose`, `SealedKey`;
  this RFC consumes those types.
- v0.2 RFC 012 (RFC 012 in sequential numbering) — *Real Digest Verification*;
  this RFC supersedes the hard-coded development digests it introduced.
- v0.3 RFC 003 — *Anti-Rollback Metadata*; consumes `KeyEpoch`.

---

## 1. Summary

Introduce a typed **Keyring** that maps `KeyPurpose` to one or more
`TrustAnchor`s, a typed **SignatureProvider** trait that performs
verify-and-detach operations using the keyring's anchors, and a stable
**KeyEpoch** model so that anchor rotation is observable to `upgraded`,
`verifyd`, and the anti-rollback path.

The keyring is `no_std`, host-testable, allocation-free, and shares
versioning conventions with the v0.2 audit and measurement formats.

The development-grade signature path in `verifyd` becomes a thin adapter that
calls `SignatureProvider::verify(...)` with the appropriate `KeyPurpose`.

---

## 2. Motivation

v0.2 verified signatures with a hard-coded development key embedded at build
time. That is acceptable for a prototype but blocks three things:

1. **Key rotation.** There is no mechanism to advance from key version *N* to
   version *N+1* and reject *N* afterwards.
2. **Per-purpose isolation.** A single key signs release manifests, rootfs
   manifests, and policy bundles. A compromise of any one workflow compromises
   all.
3. **Trust anchor pluralism.** Release-time we want a release-signing key; at
   first boot we may want a vendor key that signs the release-signing key. The
   v0.2 path has no notion of nested anchors.

This RFC introduces the minimum typed structure to fix all three without yet
requiring hardware roots.

---

## 3. Goals

```text
- A `Keyring` type that holds N anchors per KeyPurpose, generation-tagged.
- A `SignatureProvider` trait that verifies bytes-with-signature against a
  keyring entry chosen by KeyPurpose + optional anchor selector.
- A `KeyEpoch` integer that all signed records may bind to.
- An on-wire/on-disk encoding for `TrustAnchor` and `KeyringSnapshot` so the
  keyring can be persisted by storaged.
- Compatibility with the `HardwareTrustProvider::seal_key` API for protecting
  private/secret anchor material (signing keys held by the OS itself).
- All keyring mutations audited; observable in the semantic stream.
```

## 4. Non-Goals

```text
- No PKI (no certificate chains in v0.3.0; an anchor is a public key plus
  metadata, not an X.509 chain).
- No remote rotation (rotation is local + signed-by-superior-anchor; remote
  is v0.4 RFC 004 territory).
- No expiry timestamps (Fjell has no trusted clock until v0.4 RFC 003);
  KeyEpoch is the freshness primitive instead.
- No support for hybrid PQ algorithms in v0.3.0 (forward-compat reserved
  via the `algorithm` field).
```

---

## 5. External Design

### 5.1 Conceptual model

```text
                       Keyring
                       ┌───────────────────────────────────────┐
KeyPurpose             │ KeyPurpose::ReleaseVerification        │
ReleaseVerification ──►│   slot 0: TrustAnchor { algo=Ed25519,  │
                       │            epoch=3, key=..., ... }    │
                       │   slot 1: TrustAnchor { ..., epoch=2 } │  ← prior anchor
                       ├───────────────────────────────────────┤
KeyPurpose             │ KeyPurpose::PolicyVerification         │
PolicyVerification ───►│   slot 0: TrustAnchor { epoch=1, ... } │
                       └───────────────────────────────────────┘
                              ▲
                              │
                       SignatureProvider::verify(
                              purpose, message, signature)
```

Verification chooses the *highest-epoch matching anchor whose algorithm
accepts the signature*. If none match, the signature is rejected with
`SigError::NoMatchingAnchor`.

### 5.2 User-visible behavior

- `verifyd` accepts release manifests signed with anchor epoch ≥ current
  active epoch; anchors below the active epoch are rejected.
- An operator inspecting the system via the text proxy sees:

  ```text
  Keyring:
    ReleaseVerification: 2 anchor(s), active_epoch=3
    PolicyVerification:  1 anchor(s), active_epoch=1
  ```
- A failed verification emits an audit event including which purpose, which
  anchor selector, and which signature-algorithm code was attempted.

### 5.3 Rotation

Rotation is a single API:

```rust
keyring.install_anchor(purpose, new_anchor, signed_by_higher_anchor)?;
```

`install_anchor` succeeds only if `signed_by_higher_anchor` verifies against
an existing anchor with `epoch >= new_anchor.epoch - 1` and a *superior*
authority bit. (In v0.3.0 there is exactly one "superior" anchor per purpose,
the genesis dev anchor; this is sufficient for development and is replaced by
a vendor anchor in a future TPM provider profile.)

---

## 6. Data Model

### 6.1 Algorithm tag

```rust
#[repr(u8)]
pub enum SignatureAlgorithm {
    Ed25519     = 0x01,
    EcdsaP256   = 0x02,
    /// Test-only: insecure 32-byte SHA-256 keyed digest used by the
    /// DevelopmentTrustProvider. Never accepted in release profile.
    DevDigest32 = 0xFE,
}
```

### 6.2 Key epoch

```rust
pub struct KeyEpoch(pub u32);   // 0 = "no epoch / genesis"
```

`KeyEpoch` is **monotonic per purpose**. Reusing an old epoch for a new key is
a security-boundary violation and must be rejected by `install_anchor`.

### 6.3 Trust anchor

```rust
pub const ANCHOR_KEY_BYTES_MAX: usize = 64;   // Ed25519 pub=32, ECDSA-P256=65 trimmed → 64 ceiling

pub struct TrustAnchor {
    pub purpose:    KeyPurpose,
    pub algorithm:  SignatureAlgorithm,
    pub epoch:      KeyEpoch,
    pub authority:  AuthorityClass,         // Genesis | Standard
    pub key_bytes:  [u8; ANCHOR_KEY_BYTES_MAX],
    pub key_len:    u8,
    /// Optional sealed-key handle if the keyring also holds the private key
    /// (the *signing* keyring case used by attestd). For pure verification
    /// anchors this is `None`-encoded as `epoch == 0 && len == 0`.
    pub sealed_priv: Option<SealedKey>,
}

#[repr(u8)]
pub enum AuthorityClass {
    /// Genesis anchor — can sign other anchor installations for the same
    /// purpose. There is exactly one Genesis per purpose.
    Genesis = 0x01,
    /// Standard anchor — used for ordinary verification; cannot install
    /// other anchors.
    Standard = 0x02,
}
```

### 6.4 Keyring

```rust
pub const ANCHORS_PER_PURPOSE: usize = 4;

pub struct Keyring {
    /// Indexed by KeyPurpose discriminant - 1 (purpose 0x01 → row 0, ...).
    rows: [PurposeRow; 8],
}

struct PurposeRow {
    anchors: [Option<TrustAnchor>; ANCHORS_PER_PURPOSE],
    active_epoch: KeyEpoch,
}
```

The active_epoch is the *lowest* epoch the keyring will accept for that
purpose. Verifying against an anchor with a lower epoch fails even if the
signature math would otherwise check.

### 6.5 Canonical keyring snapshot

`KeyringSnapshot` is the persisted projection used by `storaged`:

```text
canonical bytes:
    MAGIC "FJLR" (4 B)
    schema_version u16 LE
    row_count u8                  (always 8 for v0.3.0)
    for each row:
        purpose_tag u8
        active_epoch u32 LE
        anchor_count u8           (0..=4)
        for each anchor:
            algorithm u8
            authority u8
            epoch u32 LE
            key_len u8
            key_bytes [u8; 64]    (zero-padded if key_len < 64)
            sealed_present u8     (0 or 1)
            if sealed_present:
                seal_purpose u8
                seal_blob_len u8
                seal_blob [u8; 96]
                seal_epoch u32 LE
    snapshot_digest Digest32 over all bytes above (with snapshot_digest=0).
```

The leading `MAGIC` and `schema_version` allow `storaged` to detect and
reject older formats during recovery scan.

---

## 7. Internal Design

### 7.1 SignatureProvider trait

```rust
pub trait SignatureProvider {
    /// Verify a signature over a message against the keyring entry chosen by
    /// `purpose`. The implementation walks anchors highest-epoch-first and
    /// returns Ok on the first successful verification at active_epoch or
    /// higher.
    fn verify(
        &self,
        keyring: &Keyring,
        purpose: KeyPurpose,
        message: &[u8],
        signature: &Signature,
    ) -> Result<KeyEpoch, SigError>;

    /// Produce a signature for `message` using the *signing* keyring entry
    /// for `purpose`. Returns `SigError::NoSigningKey` if the keyring holds
    /// only verification anchors for this purpose. attestd is the only
    /// service that holds a signing keyring.
    fn sign(
        &self,
        keyring: &Keyring,
        purpose: KeyPurpose,
        message: &[u8],
    ) -> Result<Signature, SigError>;
}
```

### 7.2 Error type

```rust
#[repr(u16)]
pub enum SigError {
    NoMatchingAnchor       = 0x0001,
    AlgorithmNotSupported  = 0x0002,
    DevDigestInReleaseMode = 0x0003,   // DevDigest32 rejected when keyring is "release"
    EpochBelowActive       = 0x0004,
    EpochReuse             = 0x0005,
    NoSigningKey           = 0x0006,
    BadSignatureLength     = 0x0007,
    SealUnavailable        = 0x0008,   // signing requires unseal but provider rejected it
    Internal               = 0xFFFF,
}
```

### 7.3 Keyring operations

```rust
impl Keyring {
    pub const fn empty() -> Self;
    pub fn install_genesis_anchor(&mut self, anchor: TrustAnchor) -> Result<(), SigError>;
    pub fn install_anchor(
        &mut self,
        anchor: TrustAnchor,
        signed_by_superior: &Signature,
        sigprov: &dyn SignatureProvider,
    ) -> Result<(), SigError>;
    pub fn anchor_count(&self, purpose: KeyPurpose) -> usize;
    pub fn active_epoch(&self, purpose: KeyPurpose) -> KeyEpoch;
    pub fn advance_active_epoch(&mut self, purpose: KeyPurpose, to: KeyEpoch)
        -> Result<(), SigError>;
}
```

`advance_active_epoch` is the operation that retires older anchors. It only
succeeds if the new epoch is `<=` the highest installed anchor's epoch.

### 7.4 DevSignatureProvider

The development implementation uses the `DevelopmentTrustProvider`'s
`SHA256(TRUST_DOMAIN || provider_id || dev_key || message)` formula with the
public 32-byte dev_key as the anchor's `key_bytes`. Verify is equality
comparison. This is intentionally insecure and **must** carry the
`DevDigest32` algorithm tag so the release-mode reject works.

### 7.5 Release-mode flag

The keyring carries a single `release_mode` bit. When `true`, `verify` rejects
`DevDigest32` with `SigError::DevDigestInReleaseMode`. The flag is flipped by
the same boot-time `enter_enforcing()` call that locks the trust-provider
registry — a single observable transition for the whole hardening surface.

---

## 8. Security Design

### 8.1 Threat model deltas

```text
Threat T-34: Adversary supplies a signature using a retired anchor (epoch
             below active).
Mitigation:  active_epoch check; verify walks anchors but rejects any
             anchor whose epoch is below active.

Threat T-35: Adversary re-uses an old epoch number for a new anchor.
Mitigation:  install_anchor rejects EpochReuse.

Threat T-36: Adversary registers a Standard anchor and then attempts to use
             it to authorise installation of another anchor.
Mitigation:  install_anchor requires the superior signature to come from a
             Genesis anchor; AuthorityClass enforced.

Threat T-37: A dev-grade anchor is left in a release-mode keyring.
Mitigation:  release_mode flag rejects DevDigest32; release-gate negative
             test proves it.

Threat T-38: Signing key is leaked from attestd via memory dump.
Mitigation:  signing private keys are held only as SealedKey; unseal requires
             the trust provider (out of attestd's address space when TPM
             lands). In v0.3.0 with DevelopmentTrustProvider this is a soft
             mitigation noted as a residual risk.
```

### 8.2 Audit emission

```text
KeyringInstallGenesis    { purpose, epoch, algorithm }
KeyringInstallAnchor     { purpose, epoch, algorithm, authority, by_epoch }
KeyringActiveEpochAdvanced { purpose, old, new }
KeyringVerifyFail        { purpose, error_code }
KeyringSnapshotPersisted { snapshot_digest }
```

### 8.3 Persistent state safety

The `KeyringSnapshot` is written through `storaged` as an append-only record.
Restoring from snapshot is part of `verifyd` startup; if the snapshot fails
its digest check, `verifyd` falls back to the build-time genesis anchors and
emits `KeyringSnapshotRecoveryFailed`.

---

## 9. Memory / Resource Design

- `TrustAnchor` size: 1+1+4+1+64+1+1+(96+8) ≈ 177 bytes worst case.
- `Keyring` size: 8 purposes × 4 anchors × 177 ≈ 5.7 KB. Suitable for static
  allocation inside `verifyd` and `attestd`.
- All operations are O(`ANCHORS_PER_PURPOSE`) = O(4); verify is bounded by
  the linear scan over the 4 anchors for one purpose.

---

## 10. Compatibility and Migration

### 10.1 Compatibility with v0.2 verifyd

- v0.2 verifyd's on-wire `release_manifest_verify_request` is unchanged.
- v0.2 verifyd's hard-coded build-time public key becomes the *Genesis anchor*
  of purpose `ReleaseVerification` with `algorithm = DevDigest32` and
  `epoch = 1`.
- v0.2 measurement records are unaffected.

### 10.2 Migration plan

| Step | Action |
|------|--------|
| 1    | Add `fjell-keyring` crate with host tests. |
| 2    | Add the genesis-anchor builder used by `verifyd` boot. |
| 3    | Refactor `verifyd` to call `SignatureProvider::verify` instead of `dev_verify`. |
| 4    | Add storage of `KeyringSnapshot` to `storaged`'s append-only log. |
| 5    | Land the release_mode flag flip during `enter_enforcing()`. |
| 6    | RFC v0.3-003 anti-rollback metadata binds to KeyEpoch — built on this layer. |

The fallback to genesis anchors on snapshot-load failure keeps boot
recoverable across the migration.

---

## 11. Test Strategy

### 11.1 Host unit tests

In `crates/fjell-keyring/src/tests.rs`:

```text
- empty_keyring_has_no_anchors
- empty_keyring_active_epoch_is_zero
- install_genesis_anchor_succeeds
- install_genesis_anchor_twice_rejected      (Genesis is unique)
- install_anchor_requires_superior_signature
- install_anchor_with_bad_superior_rejected
- install_anchor_epoch_reuse_rejected
- install_anchor_capacity_full_evicts_oldest_or_errors  (deferred: pick one)
- verify_picks_highest_epoch_first
- verify_rejects_anchor_below_active_epoch
- verify_returns_actual_epoch_used
- verify_dev_digest_in_release_mode_rejected
- verify_no_matching_anchor_returns_error
- sign_without_signing_key_returns_no_signing_key
- sign_then_verify_round_trip
- advance_active_epoch_monotonic
- advance_active_epoch_above_installed_rejected
- snapshot_serialize_then_load_round_trip
- snapshot_with_bad_magic_rejected
- snapshot_with_bad_digest_rejected
- snapshot_truncated_rejected
```

Target: ≥ 22 tests.

### 11.2 QEMU negative tests

| Marker                                              | Profile  |
|-----------------------------------------------------|----------|
| `NEG:KEYRING:RETIRED_EPOCH_REJECTED`               | keyring  |
| `NEG:KEYRING:DEV_DIGEST_IN_RELEASE_REJECTED`       | keyring  |
| `NEG:KEYRING:UNAUTHORISED_INSTALL_REJECTED`        | keyring  |
| `NEG:KEYRING:EPOCH_REUSE_REJECTED`                 | keyring  |
| `NEG:KEYRING:BAD_SUPERIOR_SIGNATURE_REJECTED`      | keyring  |

### 11.3 Fuzz target (deferred to v0.6 RFC 003)

```text
- fuzz target: KeyringSnapshot deserialiser. Input: arbitrary bytes ≤ 16 KB.
  Property: load never panics; either returns Ok with a valid Keyring or
  returns a defined error code.
```

---

## 12. Acceptance Criteria

```text
- fjell-keyring crate exists and builds host + cross.
- ≥ 22 host unit tests pass.
- verifyd refactored to use SignatureProvider trait; the v0.2 verify smoke
  test still passes.
- release_mode flag transitions atomically with trust-provider
  enter_enforcing().
- All 5 QEMU keyring negative markers are green in CI.
- KeyringSnapshot round-trips through storaged.
- New ADR ADR-v0.3-002 filed (keyring boundary).
```

---

## 13. Documentation Requirements

```text
docs/src/architecture/v0.3-002-keyring.md
docs/src/development/v0.3-002-keyring.md
docs/src/verification/v0.3-002-keyring-invariants.md
docs/src/adr/v0.3-002-keyring-boundary.md
docs/src/format/keyring-snapshot.md            — on-disk format
```

---

## 14. Open Questions

1. **ANCHORS_PER_PURPOSE = 4** — is four enough for production rotation
   choreography? In a regulated rotation (sign-A → install-B → sign-B →
   retire-A → install-C → sign-C → retire-B) you need ≥ 3 at any time. Four
   gives headroom. If a future profile needs more, bump the constant in a
   v0.3.x RFC.
2. **DevDigest32 size** — 32 bytes is half of `SIGNATURE_LEN`. We accept this
   for v0.3.0 to keep the type uniform. If a 64-byte algorithm is added later
   the signature length field already encodes the actual length.
3. **Genesis anchor rotation** — currently impossible (Genesis is install-once).
   In v0.4 the secure-transport channel may bring a vendor-signed genesis
   replacement; tracked in v0.4 RFC 004.

---

## 15. Release Gate (RFC-local)

```text
- Crate landed, tests green, verifyd refactored, release-mode flag wired.
- 5 negative markers in CI.
- ADR-v0.3-002 Accepted.
- CHANGELOG entry filed for landing version.
```
