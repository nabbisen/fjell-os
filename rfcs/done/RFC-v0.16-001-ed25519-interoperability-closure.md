# RFC-v0.16-001: Ed25519 Interoperability Closure

**Status:** Implemented (v0.16.0)
**Milestone:** v0.16 — Validation Closure
**Blocks:** v1.0.0 tag (architect review RB-01)

---

## 1. Problem

The v0.9–v0.15 handoff (§0.3) reported that two RFC 8032 §7.1 Test
Vector 1 tests in `fjell-sig-ed25519` had been **removed rather than
reconciled**:

- `from_seed_matches_tv1_public` — derive public key from the TV1 seed.
- `sign_tv1_produces_tv1_sig` — sign the empty message, expect TV1 sig.

The reported symptom: seed `9d61b19d…` produced public key
`b210cde6…` under `ed25519-dalek` 2.2.0, not the expected `d75a9801…`.
The handoff flagged this as the highest-priority release blocker
(R1/RB-01) because it left the bundle-signing path's RFC-8032
conformance unproven.

## 2. Investigation

Three independent Ed25519 implementations were used as oracles:

| Implementation | Backend | seed `9d61…2c44da4e…` → pubkey |
|----------------|---------|-------------------------------|
| `ed25519-dalek` 2.2.0 | pure Rust | `b210cde6…` |
| pyca/cryptography | OpenSSL | `b210cde6…` |
| PyNaCl | libsodium | `b210cde6…` |

All three agreed. Three independent backends cannot share a key-derivation
bug, so the implementations were correct and the **input was wrong**.

Comparing the in-tree `TV1_SECRET` constant against the canonical RFC 8032
§7.1 text (rfc-editor.org, cross-checked against the Botan and
asecuritysite renderings):

```
canonical: 9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60
in-tree:   9d61b19deffd5a60ba844af492ec2c44da4e810f7098100111284d23a5d36eca
                                          ^^ divergence begins at byte 15
```

The in-tree seed had been corrupted from byte 15 onward — a transcription
error. The implementations had faithfully derived the public key for the
**wrong seed**. There was never a cryptographic defect.

## 3. Resolution

1. `TV1_SECRET` corrected to the canonical RFC 8032 §7.1 value.
2. Both removed tests restored:
   - `from_seed_matches_tv1_public` — **passes**.
   - `sign_tv1_produces_tv1_sig` — **passes**.
3. Cross-implementation evidence committed to this RFC: the corrected
   seed produces `d75a9801…` (canonical TV1 public key) and signs the
   empty message to `e5564300…` (canonical TV1 signature), matching the
   RFC text exactly, and matching OpenSSL and libsodium output.

## 4. Conformance statement

The Fjell Ed25519 backend (`fjell-sig-ed25519`) is RFC 8032 conformant
on all three paths:

- **Derive:** seed → public key matches RFC 8032 §7.1 TV1.
- **Sign:** empty-message signature matches RFC 8032 §7.1 TV1 byte-for-byte.
- **Verify:** the canonical TV1 (public key, signature) pair verifies.

Signatures produced by `cargo xtask sign-bundle` are byte-identical to
those produced by OpenSSL and libsodium for the same key and message.
This closes threat-model item T10's defence claim and resolves RB-01.

## 5. Errata raised

This investigation supersedes the handoff §0.3 and §6.1 claim that the
discrepancy was "reproducible and unexplained." It is now explained:
operator transcription error in a test constant. Recorded in
`docs/rfcs/ERRATA.md` as E-001 (see RFC-v0.16-004).

## 6. Lesson

Test vectors copied by hand are themselves a defect surface. v0.16-004
adds a standing requirement that cryptographic test vectors be
cross-verified against at least one independent implementation at
authoring time, with the verification command committed alongside.
