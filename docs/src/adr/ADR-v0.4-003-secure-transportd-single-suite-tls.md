# ADR-v0.4-003 — secure-transportd: Single-Suite TLS 1.3 with In-Process Crypto

**Status:** Accepted  
**Date:** 2026-05-19 (v0.4.0, RFC v0.4-003)

---

## Context

Control-plane communications (update fetch, diagnostics push, attestation)
require confidentiality and mutual authentication.  Fjell has no_std
constraints, no OS TLS library, and a strong preference for auditable,
minimal code over general-purpose flexibility.

## Decision

`secure-transportd` uses exactly one cipher suite: **TLS_AES_128_GCM_SHA256**
with **X25519** key exchange.  No cipher suite negotiation; the service
rejects connections offering anything else.

The crypto primitives live in the `fjell-sxt-crypto` crate, which is
`#![no_std]` and host-testable:

- **AES-128-GCM**: table-free, constant-time reference implementation.
  NIST SP 800-38D test vectors pass.

> **Correction, 2026-10-21.** The AES-128-GCM bullet is wrong on both counts.
> `fjell-sxt-crypto/src/aes128.rs` holds a 256-byte S-box and indexes it with
> state bytes in `SubBytes` and with key bytes in the key schedule, which on a
> cache-bearing target leaks key material through cache timing. The crate's own
> header has disclosed this since RFC-v0.7.3-002 — it requires an explicit
> `crypto-profile-development` feature to compile and says the `constant-time`
> claim "has been removed" — but the removal missed this record and the
> `aes128.rs` module header, so both went on asserting it. The decision this
> ADR records, one suite with no negotiation, is unaffected. The bullet is left
> as written, with the correction beside it, because this is a published
> decision record.
- **X25519**: 5×51-bit limb Montgomery ladder with the standard djb
  `fe_invert` addition chain (z^(2^255−21)).  Verified against the Python
  `cryptography` library.
- **HKDF-SHA256**: RFC 5869 extract + expand, with correct length-bounded
  intermediate buffer.  RFC 5869 A1 vector passes.
- **TLS 1.3 state machine**: pure transitions (Closed → ClientHelloSent →
  ServerHelloReceived → HandshakeComplete → AppData), no wire-format parsing
  in-crate.

Certificate verification against pinned trust anchors is stubbed in v0.4.0
and will be completed in v0.5.0 when anchor provisioning is defined.

## Consequences

- Minimal attack surface: single suite, no downgrade, no negotiation.
- All crypto code is unit-tested on the host; QEMU tests are integration-only.
- No runtime crypto library dependency; the kernel's randomness source
  is used only for the X25519 ephemeral key (injected via cap-broker).
- Pinned trust anchors replace CA chains; this is appropriate for a
  closed-fleet embedded OS but limits use in open-internet scenarios.
