# RFC-v0.11-002 — Ed25519 Signature Provider and Real Crypto Backend

**Status:** Implemented (v0.11.0)
**Target version:** v0.11.0
**Parent:** v0.11-001.
**Cross-refs:** RFC v0.3-002 (SignatureProvider trait), RFC v0.7.3-002
    (production-mode gate).

## 1. Problem

`SignatureProvider` (RFC v0.3-002) exists as a trait. The only
implementation today is a stub returning canned bytes. The
production-mode gate (RFC v0.7.3-002) refuses to enter production with
the stub provider — which is correct, but means no Fjell release has
ever entered production mode.

v0.11 ships the first real backend: Ed25519 (RFC 8032) over an audited
no-std-compatible Rust crate.

## 2. Why Ed25519

Considered alternatives:

| Algorithm | Decision | Reason |
|-----------|----------|--------|
| **Ed25519** | **Chosen** | Deterministic (no nonce-reuse risk), fast, small keys, no_std-compatible crates exist, well-understood |
| ECDSA P-256 | Rejected | Nonce reuse is catastrophic; harder in no_std |
| ECDSA secp256k1 | Rejected | Bitcoin-flavoured baggage; no operational benefit |
| RSA | Rejected | Key size, signature size, dependency weight |
| Ed448 | Rejected | Limited ecosystem support; minor security upgrade not worth the friction |
| ML-DSA (Dilithium) | Deferred | Post-quantum; revisit as v1.x hybrid mode |

## 3. Implementation

### 3.1 Crate selection

Candidates evaluated for the no_std signing path:

- `ed25519-dalek` (with `default-features = false`) — preferred. Well
  audited, widely used, supports `no_std + alloc`.
- `ed25519-compact` — fully no_std-no-alloc. Considered as fallback if
  the kernel-side verifier needs to run without `alloc`.
- `dalek-cryptography/ed25519` — superseded by `ed25519-dalek`.

The host signing tool uses `ed25519-dalek` with full `std`. The
kernel-side verifier uses whichever crate landing review prefers; the
choice is local to the verifier and does not affect the wire format.

### 3.2 New crate: `fjell-sig-ed25519`

```text
crates/fjell-sig-ed25519/
  Cargo.toml         — feature: `std` (host signer), default no_std verifier
  src/
    lib.rs           — re-exports
    signer.rs        — std-only; sign() against a SecretKey
    verifier.rs      — no_std; verify() against a PublicKey
    keys.rs          — Ed25519PublicKey, Ed25519SecretKey newtypes
    provider.rs      — impl SignatureProvider
  tests/
    rfc8032_test_vectors.rs   — IETF test vectors
```

### 3.3 SignatureProvider impl

The provider implements the v0.3-002 trait. New methods are not added.
Existing trait shape:

```rust
pub trait SignatureProvider {
    fn sign(&self, key_id: KeyId, msg: &[u8]) -> Result<Signature, SigError>;
    fn verify(&self, key_id: KeyId, msg: &[u8], sig: &Signature) -> Result<(), SigError>;
    fn public_key(&self, key_id: KeyId) -> Result<PublicKey, SigError>;
}
```

`Ed25519Provider::new(keyring)` consumes a `Keyring` from
`fjell-keyring` and routes each `KeyId` to the matching Ed25519 secret.

### 3.4 Key format

Ed25519 keys are 32 bytes (private seed) + 32 bytes (public). The
`Ed25519SecretKey` newtype implements `ZeroizeOnDrop`.

Key files use the RFC 8410 PKCS#8 format for interop with standard
tools (`openssl pkey`, `ssh-keygen -t ed25519` after conversion).
v0.11 supports only the raw 32-byte form for simplicity; PKCS#8 import
is a v0.11.x patch if needed.

## 4. RFC 8032 test vectors

The RFC 8032 §7.1 test vectors must all pass. Tests:

- Round-trip: sign then verify each test vector key+msg pair.
- Tampered signature rejected.
- Wrong public key rejected.
- Empty message accepted (per spec).

Plus property tests (10 properties × 1000 cases) covering:

- `verify(sign(m)) == Ok(())` for arbitrary m.
- `verify(tamper(sign(m)))` always errs.
- Determinism: `sign(m)` produces identical bytes across calls.

## 5. Integration with existing trust spine

- `fjell-keyring` learns to instantiate `Ed25519Provider` from its
  in-memory key material.
- The production-mode gate (RFC v0.7.3-002) accepts `Ed25519Provider`
  as the first non-stub provider that satisfies its requirements.
- `fjell-bundle-format::verify_bundle` continues to verify the bundle
  digest; signature verification is a separate step at the installer
  layer (see v0.11-003).

## 6. Acceptance criteria

1. `crates/fjell-sig-ed25519/` exists and builds for the workspace
   target including no_std verifier.
2. All RFC 8032 §7.1 test vectors pass.
3. At least 10 property tests pass with 1000 cases each.
4. `Ed25519Provider` is wired into `fjell-keyring`.
5. Production-mode gate accepts `Ed25519Provider` and refuses to
   enter production while the stub provider is registered.
6. Unsafe-audit gate remains at 0 missing.
7. `cargo xtask test-all` passes.

## 7. Out of scope

- PKCS#8 key import (v0.11.x if needed).
- Hardware key storage (v0.12 + board choice).
- Multi-signature schemes (v0.11.x or v0.14).
- Post-quantum hybrid mode (research track).
