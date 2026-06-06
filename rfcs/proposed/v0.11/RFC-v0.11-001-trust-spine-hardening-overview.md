# RFC-v0.11-001 — Trust Spine Hardening Overview

**Status:** Proposed
**Target version:** v0.11.0
**Parent:** RFC 061 §10 (roadmap).
**Cross-refs:** v0.11-002 through v0.11-005.

## 1. Purpose

v0.3 shipped the *interfaces* of the trust spine — `HardwareTrustProvider`,
`Keyring`, `KeyEpoch`, anti-rollback metadata, attestation profile v2. v0.7
shipped the *enforcement gates* — production-mode policy, NodeIdentity
trust-mode fail-closed. What is missing is the *production-grade content*
behind those interfaces: a real signing backend, a real key rotation
discipline, a real replay defence.

v0.11 closes that gap. The architectural surface does not change. The
SDK's API revision does not change. What changes is the trust posture
of every release artefact produced after v0.11 ships:

- Bundles are signed by a real Ed25519 key whose provenance is documented.
- Key rotation has explicit semantics, not "redeploy and hope".
- Revoked keys produce a refusal trace, not silent acceptance.
- Replayed attestations are detected and refused.

## 2. Composition

| RFC | Title | Deliverable |
|-----|-------|-------------|
| v0.11-001 | This overview | Coordination |
| v0.11-002 | Ed25519 Signature Provider and Real Crypto Backend | Real `SignatureProvider` impl |
| v0.11-003 | Bundle Signing Pipeline and Key Material Management | `cargo xtask sign-bundle` |
| v0.11-004 | Keyring Rotation and Key Revocation Records | KeyEpoch transitions + revocation log |
| v0.11-005 | Replay Cache and Attestation Freshness | Nonce-tracked freshness check |

These RFCs are tightly coupled but independently testable.

## 3. What v0.11 explicitly does *not* include

- Hardware secure-element integration (TPM, OP-TEE, PSA). The
  `HardwareTrustProvider` trait already accommodates it; landing a
  hardware backend is deferred until v0.12 has chosen a real board with
  a known secure-element story.
- Post-quantum signatures (ML-DSA / Dilithium). v0.11 ships classical
  Ed25519; a hybrid mode is a v1.x research-track candidate.
- Cross-host distributed signing or hardware security modules. Bundle
  signing in v0.11 uses an offline host with a local key file.
- New attestation record formats. AttestationRecordV2 is the v1.0
  format; v0.11 hardens its validators, not its shape.

## 4. Why these four, in this order

1. **Real backend first** (002): without a real cryptographic primitive,
   every subsequent guarantee is rhetorical.
2. **Signing pipeline second** (003): once a real signer exists, the
   release process must use it deterministically.
3. **Rotation third** (004): a signing pipeline without rotation makes
   compromise irrecoverable.
4. **Replay defence last** (005): replay protection assumes both signed
   evidence and key generation in motion; landing it earlier risks
   building against a moving target.

## 5. Release criteria

v0.11.0 may be tagged when:

1. All four sub-RFCs (002–005) are merged to `done/`.
2. `cargo xtask sign-bundle` produces a bundle with a valid Ed25519
   detached signature; `verify-bundle` accepts it; tampered bundles are
   rejected with a documented error.
3. A keyring rotation test scenario boots cleanly: epoch N → N+1 with
   bundles signed under both, then with N revoked.
4. A replay scenario refuses a re-submitted attestation with a clear
   audit record.
5. The Trust Report (RFC 061 §6) gains a "Signature evidence" subsection
   listing signing key fingerprints and rotation timestamps.
6. `cargo xtask test-all` passes including all new replay and rotation
   negative tests.

## 6. Risk register

| Risk | Mitigation |
|------|------------|
| Choice of crypto crate ages badly | Wrap behind `SignatureProvider`; swap is mechanical |
| `no_std` constraints limit library choice | RFC-v0.11-002 §3 lists the no_std-clean candidates |
| Key file handling becomes accidentally lax | RFC-v0.11-003 §5 mandates encrypted-at-rest + tooling refusal |
| Replay cache memory blow-up | RFC-v0.11-005 §4 specifies bounded size and eviction policy |
| Existing tests break under signed bundles | v0.11-003 §6 keeps a test-mode key with the same code path |

## 7. Out of scope

Beyond §3 non-goals: this RFC explicitly does *not* address
- governance of the signing key (who holds it, quorum, ceremony);
  v0.15 release-discipline territory.
- hardware-rooted attestation. Software-rooted is acceptable for v0.11;
  hardware comes with v0.12's board choice.
