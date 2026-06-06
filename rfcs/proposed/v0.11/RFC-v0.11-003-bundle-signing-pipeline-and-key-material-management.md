# RFC-v0.11-003 — Bundle Signing Pipeline and Key Material Management

**Status:** Proposed
**Target version:** v0.11.0
**Parent:** v0.11-001.
**Cross-refs:** RFC v0.9-004 (bundle format), v0.11-002 (Ed25519 provider).

## 1. Problem

RFC v0.9-004 defined the bundle format with a `bundle_digest` but
deferred the signing pipeline. v0.11-002 now provides a real signer.
This RFC connects them: producing a signed bundle from source, storing
the signing key safely, and verifying signatures at install time.

## 2. Wire format addition

A signed bundle adds a detached signature container alongside the
bundle. Two file artefacts per release:

```
fjell-hello.bundle      — ServiceBundle wire bytes (RFC v0.9-004)
fjell-hello.bundle.sig  — SignedManifest (this RFC)
```

`SignedManifest` is a small TLV record:

```text
magic:        u32  = 0xFJ51 5301  ("FJSIG01")
schema:       u16  = 1
key_id:       [u8; 16]            — Keyring KeyId
sig_alg:      u8   = 1            — 1 = Ed25519 (RFC 8032)
reserved:     u8                  — 0
bundle_digest:[u8; 32]            — must equal bundle.header.bundle_digest
signed_at_ns: u64 (LE)            — signer's clock at sign time
signature:    [u8; 64]            — over canonical message
```

Canonical signed message:

```text
SIG_DOMAIN || key_id || sig_alg || bundle_digest || signed_at_ns_be8
```

where `SIG_DOMAIN = b"FJELL-BUNDLE-SIG-V1"`.

## 3. Builder workflow

A new xtask subcommand:

```
cargo xtask sign-bundle \
    --bundle  path/to/fjell-hello.bundle  \
    --key     path/to/signing.key         \
    --key-id  <hex16>                     \
    --out     path/to/fjell-hello.bundle.sig
```

The signer:
1. Reads the bundle, computes its digest.
2. Asserts the stored `bundle_digest` matches (refuses tampered bundles).
3. Loads the signing key (zeroized on drop).
4. Builds the canonical message.
5. Calls `Ed25519Provider::sign`.
6. Writes the `SignedManifest`.

A companion `verify-bundle-sig` exists for diagnostic use and is the
same code path the installer runs.

## 4. Installer workflow

`fjell-bundle-format::verify_signed_bundle(&bundle, &sig, trust_anchors)`:

1. Re-compute the bundle digest; assert equality.
2. Find `key_id` in `trust_anchors`.
3. Reject if the key is revoked (RFC-v0.11-004 §3).
4. Reconstruct the canonical message.
5. Call the `SignatureProvider::verify`.
6. On success, return the resolved `KeyEpoch` and `signed_at_ns`.

Failure produces a typed error: `Tampered`, `UnknownKey`, `RevokedKey`,
`SigVerifyFailed`, `EpochExpired`.

## 5. Key material management

### 5.1 At rest

The signing key file is encrypted at rest using a passphrase-derived
key (Argon2id). The tool refuses to read an unencrypted key file. A
companion command:

```
cargo xtask key gen   --out signing.key            (interactive passphrase)
cargo xtask key show  --in  signing.key            (prints public part only)
cargo xtask key encrypt --in raw.key --out signing.key
```

### 5.2 In memory

`Ed25519SecretKey` implements `ZeroizeOnDrop`. The signer holds the
key only for the duration of one `sign()` call.

### 5.3 Provenance

Every produced `.bundle.sig` carries `signed_at_ns`. The Trust Report
(RFC 061 §6) records key fingerprint + signing time for each artefact
in a release.

## 6. Compatibility with test mode

CI must continue to run end-to-end without operator interaction. A
test-mode key is generated at workspace root build time and used by
all CI signing. The `production-mode` gate refuses to accept the
test-mode key id (the test key id is reserved: `0x00..00`).

## 7. CI integration

- `test-all` adds a `sign-and-verify` step that signs the current
  bundles with the test key and verifies them.
- A negative test: tamper a byte in `.bundle`, the signature
  verification must fail with `Tampered`.
- A negative test: tamper a byte in `.bundle.sig`, verification fails
  with `SigVerifyFailed`.

## 8. Acceptance criteria

1. `cargo xtask sign-bundle` produces a valid `.bundle.sig`.
2. `cargo xtask verify-bundle-sig` accepts the signed bundle.
3. Tampered bundle → `Tampered`. Tampered sig → `SigVerifyFailed`.
   Wrong key id → `UnknownKey`. Revoked key → `RevokedKey`.
4. `cargo xtask key gen` produces an encrypted key file. The signer
   refuses an unencrypted key file.
5. `Ed25519SecretKey` is `ZeroizeOnDrop`.
6. `test-all` includes the signing + tampering tests.
7. The production-mode gate (RFC v0.7.3-002) refuses the test key id.

## 9. Out of scope

- Hardware-key signing (deferred to v0.12 board choice).
- Multi-signature / quorum-signed releases (deferred to v0.13 or v0.14).
- Cross-signature with external roots of trust (research track).
- Time-stamping authority integration.
