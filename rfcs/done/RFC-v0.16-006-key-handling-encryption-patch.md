# RFC-v0.16-006: Key Handling — Encryption at Rest

**Status:** Implemented (v0.16.0)
**Milestone:** v0.16 — Validation Closure
**Addresses:** architect review H-01; errata E-002

## Problem

Signing keys were written as plaintext (`FJKY` magic). RFC-v0.11-003 §5
claimed Argon2id-derived encryption at rest; that claim was drift.

## Change

New module `crates/fjell-tools/src/key_crypto.rs` implements an encrypted
`FJK2` key format:

```
magic "FJK2" | version | kdf_id | salt[16] | nonce[12] | pubkey[32] | ct+tag[48]
```

- Key derivation: **Argon2id** (default params) from an operator passphrase.
- Sealing: **AES-256-GCM** over the 32-byte Ed25519 seed.
- Public key stored cleartext so `key show` needs no passphrase.

Passphrase resolved from `--passphrase` or `FJELL_KEY_PASSPHRASE`.
`key gen` encrypts by default; the legacy plaintext path is retained only
behind an explicit `--insecure-plaintext` flag for CI fixtures. Reading
transparently supports both formats.

## Verification

- 6 unit tests: round-trip, wrong-passphrase rejection, GCM tamper
  detection, bad-magic rejection, cleartext-pubkey readability, salt
  uniqueness.
- End-to-end: `key gen` (encrypted) → `key show` → `sign-bundle`
  (decrypts) → `verify-bundle-sig` → PASS. Wrong passphrase correctly
  refused at sign time.

## Residual

`ZeroizeOnDrop` byte-level guarantee on the in-memory seed is still not
independently verified (handoff §4.4); tracked as a v1.x hardening item,
listed in v1.0 limitations.
