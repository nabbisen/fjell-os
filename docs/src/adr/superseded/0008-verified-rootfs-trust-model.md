# ADR 0008: Verified immutable rootfs and signed artifact model

**Status:** Superseded — see [ADR-0008 Verified Immutable Rootfs](./0008-verified-immutable-rootfs.md) (RFC 045 rename)  
**Date:** 2026-05-12  
**Milestone:** M7

---

## Context

M7 introduces a "Verified Immutable System" milestone.  The goal is that service
images and policy documents can only be loaded if they carry a valid signature, and
that the rootfs namespace is read-only after boot.

The threat model at v0.1.0 is a development-grade trust model: the signing key is a
known constant embedded in the kernel, not a hardware-backed key.  Production secure
boot (hardware root of trust, TPM, DICE) is deferred to v1.0.

---

## Decision

### fjell-verify-format

`SignedObject<T>` wraps an arbitrary payload with a `DevSignature` struct:

```rust
pub struct DevSignature {
    pub key_id:  [u8; 4],
    pub digest:  [u8; 32],
    pub sig:     [u8; 32],  // placeholder bytes in M7
}
pub const DEV_SIGNATURE_VALID: DevSignature = DevSignature {
    key_id:  [0xDE, 0xAD, 0xBE, 0xEF],
    digest:  [0xAA; 32],
    sig:     [0x55; 32],
};
```

`SignedObject::verify_dev()` returns `Ok(())` iff `self.signature == DEV_SIGNATURE_VALID`.

**Known limitation (RB-06):** Verification is a byte comparison against a compile-time
constant.  The `digest` field is not computed from the payload; `key_id` is not looked
up in a key store; and the `sig` field does not use Ed25519 or any real cryptographic
primitive.  This makes the verification gate meaningful as a structural check
(a bundle without the constant signature is rejected), but not as a cryptographic
guarantee.

**RFC 012 (deferred to M8):** Replace with Ed25519 + canonical CBOR (COSE Sign1-style).

### fjell-rootfs-format

`RootfsNamespace` holds a fixed-capacity array of `RootfsObject` entries, each
recording a service image name (`ServiceName`) and a `Hash32` digest placeholder
(`[0xBB; 32]`).  The read-only property is enforced by the type system
(`RootfsNamespace` has no `&mut` methods after `build()`).

**Known limitation:** Digest values are fixed sentinel bytes, not computed from service
image binaries.  `rootfsd` (stub) does not verify object contents against the manifest.

### fjell-snapshot-format

`SystemSnapshot` captures slot state, rootfs hash, policy hash, and release hash at a
named point in the system lifecycle (Boot, PreUpgrade, PostConfirmation, Rollback).
Hash fields use 8-byte shortened sentinels (`REL_HASH`, `RFS_HASH`, `POL_HASH`) in M7;
canonical SHA-256 is deferred to M8.

### verifyd service

`fjell-verifyd` is a stub binary in M7; all verification logic runs inline in
`fjell-init`.  The service signature is structurally verified (constant comparison)
before any slot is staged or confirmed.  Invalid signatures cause the upgrade sequence
to abort.

---

## Consequences

- Bundles without `DEV_SIGNATURE_VALID` are rejected by the verify path, providing
  smoke-level protection.
- The rootfs namespace is logically immutable after `build()` in M7.
- Cryptographic trust requires RFC 012 (Ed25519 / COSE Sign1), targeted for M8.
- Production deployments must not use the development signature model.
