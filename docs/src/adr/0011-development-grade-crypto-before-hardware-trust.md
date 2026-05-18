# ADR-0011 — Development-Grade Crypto Before Hardware Trust

**Status:** Accepted  
**Date:** 2026-05-17 (v0.1.4, RFC 045) — captures decision made in M7

---

## Context

Fjell OS needs signature verification for release bundles, policy, and
rootfs integrity. The design question is: should the first iteration wait
for hardware-rooted trust (TPM, DICE, fused key) before implementing
verification, or ship a development-grade stand-in now?

---

## Decision

Ship a **development-grade crypto stand-in** in v0.1.0:

- Verification uses SHA-256 keyed under a fixed constant
  (`dev-attest-m8-01` in the kernel source).
- The signature format (`DevSignature`) is structurally identical to
  what a production hardware-backed signature would use: `key_id`,
  `digest`, `sig` fields.
- The key and signature are not cryptographically meaningful against an
  adversary who can read the source tree.

This decision is time-boxed: development-grade crypto is **only** for
v0.1.x. v0.3 (Hardware Trust Abstraction) replaces it.

---

## Consequences

- Verification logic and data shapes are real and can be tested.
- The `fjell-verify-format`, `fjell-attestation-format`, and
  `fjell-config-format` crates use the same `SignedObject<T>` wrapper
  they will use with a hardware-backed key.
- Upgrade and recovery paths are exercised with real data shapes.
- An attacker who can read the source can forge any bundle. This is
  explicitly documented in the threat model (§12, Known Weaknesses #9).
- CI negative tests verify that *unsigned* or *tampered* bundles are
  rejected even with the development key.

---

## Security Boundary Impact

This decision introduces a known weakness: the verification boundary is
not cryptographically sound against an attacker with source access.

The boundary remains useful for:
- Testing the data pipeline (sign → verify → use) end-to-end.
- Verifying that unsigned bundles are rejected.
- Verifying that tampered digests are rejected.

It does **not** verify that the signer is trusted; only that the bundle
was not tampered with *after* the development-key signature was applied.

---

## Deferred Work

- Hardware-rooted key (TPM / DICE / PUF): v0.3.
- Production secure boot: v0.3+.
- Key rotation: no current milestone.
- Multi-key policy: no current milestone.

---

## Related RFCs

- RFC 012 (Real Digest Verification, M7)
- RFC 028 (ABI Inventory, v0.1.2)
- RFC 027 (Threat Model, v0.1.2) — §12 Known Weaknesses #9
