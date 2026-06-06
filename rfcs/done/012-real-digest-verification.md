# RFC 012: Real digest verification in fjell-verify-format

**RFC ID:** 012  
**Status:** Implemented (v0.1.0)
**Affects:** `crates/fjell-verify-format/src/lib.rs`

## Problem (RB-06)

`SignedObject::verify_dev()` is a byte comparison against a 32-byte constant.
Object digest, kind, key purpose are not checked.
Rootfs object digest mismatch is not detectable.
Snapshot digest uses fixed sentinel strings.

## Proposed fix (M8, aligned with deep research report)

Use **Ed25519 + canonical CBOR** (COSE Sign1):
- `ed25519-compact` crate for `no_std` Ed25519 verification.
- Manifest payload serialised to canonical CBOR using `minicbor`.
- `verify_dev()` replaced by `verify(public_key: &[u8; 32]) -> VerificationResult`.
- Digest = SHA-256 of canonical CBOR payload.

For rootfs: each `RootfsObject` carries a `sha256: [u8; 32]` digest.
`rootfsd::lookup()` computes SHA-256 of retrieved object and compares.

For snapshot: `SnapshotDigest` fields computed as truncated SHA-256 of relevant
state rather than fixed strings.

## Defer condition

Requires introducing crypto crates (`ed25519-compact`, `sha2` or equivalent).
Implement in M8 evidence/attestation plane.
