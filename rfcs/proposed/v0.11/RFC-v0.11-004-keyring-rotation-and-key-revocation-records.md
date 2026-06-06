# RFC-v0.11-004 — Keyring Rotation and Key Revocation Records

**Status:** Proposed
**Target version:** v0.11.0
**Parent:** v0.11-001.
**Cross-refs:** RFC v0.3-002 (KeyEpoch model), RFC v0.7.4-003 (auth hardening).

## 1. Problem

The `KeyEpoch` model (RFC v0.3-002) exists at the type level but no
operational procedure defines how an epoch advances, how a key
becomes revoked, or how a verifier learns of revocation. Without
explicit rotation:

- Signing keys age indefinitely.
- Compromise has no recovery procedure.
- The trust anchor distribution story is unwritten.

v0.11 lands the operational semantics.

## 2. KeyEpoch state machine

```text
        ┌─────────┐    advance     ┌─────────┐
        │ Active  │───────────────►│ Retired │
        │ epoch=N │                │ epoch=N │
        └────┬────┘                └────┬────┘
             │                          │
             │ revoke                   │ revoke
             ▼                          ▼
        ┌─────────────┐            ┌─────────────┐
        │ Revoked     │            │ Revoked     │
        │ epoch=N     │            │ epoch=N     │
        │ reason=...  │            │ reason=...  │
        └─────────────┘            └─────────────┘
```

Transitions:

| From | To | Trigger | Permitted by |
|------|-----|---------|--------------|
| Active(N) | Retired(N) | Operator advances to N+1 | New Active(N+1) installed |
| Active(N) | Revoked(N) | Compromise / loss / rotation policy | Revocation record signed |
| Retired(N) | Revoked(N) | Compromise discovered after rotation | Revocation record signed |

Once Revoked, a key never re-enters service. New keys get new epoch
numbers.

## 3. Revocation record

A signed, persistable record:

```text
magic:        u32  = "FREV"
schema:       u16  = 1
key_id:       [u8; 16]
epoch:        u32
reason_code:  u16        — 1=compromised, 2=rotated, 3=lost, 4=ceremony
revoked_at:   u64        — wall-clock ns (advisory)
signer_key:   [u8; 16]   — key authorising the revocation
signature:    [u8; 64]   — over (magic..signer_key) by signer_key
```

`signer_key` must itself be Active and must hold revocation authority
(a new `CapRights::KEY_REVOKE` bit added in v0.11; without it a key
may not revoke others).

## 4. Trust anchor store

Verifiers maintain a small persisted set of trust anchors:

```text
TrustAnchor {
    key_id:      [u8; 16],
    epoch:       u32,
    pubkey:      [u8; 32],   — Ed25519 public bytes
    state:       u8,         — 1=Active, 2=Retired, 3=Revoked
    revocation:  Option<RevocationRecord>,
}
```

Persisted via `storaged` in a fixed-size table; record size bounded so
storage cost is deterministic.

A verifier:
1. Looks up `key_id` in the trust-anchor store.
2. If `state == Revoked`, refuses with `RevokedKey`.
3. If `state == Retired`, accepts only within a grace window (default
   30 days from `revoked_at`; configurable per profile).
4. If `state == Active`, accepts.

## 5. Rotation procedure

The operator playbook:

1. Generate new key (`cargo xtask key gen --epoch N+1`).
2. Distribute new public key to all verifiers as `Active(N+1)`.
3. Mark previous key `Retired(N)` once at least one verifier has
   acknowledged N+1.
4. Sign all future bundles with N+1.
5. After grace window, optionally revoke N for rotation
   (reason_code=2).

Each step is itself a bundle / record that flows through the same
trust spine — there is no out-of-band channel.

## 6. Replay-safety of revocation

Revocations are themselves replay-targets: an attacker who suppresses
a revocation record gains additional days of compromised-key validity.
RFC-v0.11-005 §3 handles attestation replay; revocation distribution
freshness is mitigated here by:

- Each verifier records `latest_revocation_seq` and refuses to accept
  a lower sequence.
- Revocation records carry a monotonic per-issuer counter.
- A verifier that has not seen a fresh revocation update within its
  configured window enters a degraded "stale-trust-anchors" state and
  may refuse to install new bundles (policy-driven).

## 7. Audit and explainability

Every state transition emits a semantic intent record on a reserved
catalog range (allocated as part of this RFC). The Trust Report
(RFC 061 §6) gains:

- For each trust anchor: state, epoch, last transition.
- Recent revocations (within reporting window) with reason codes.

## 8. Acceptance criteria

1. `KeyEpoch` carries the new `Active`/`Retired`/`Revoked` state.
2. `RevocationRecord` serialises/deserialises round-trip; signature
   validates under the issuer's key.
3. Rotation playbook reproducible against a 3-node fleet (the v0.10-005
   reference) with all three verifiers updating in order.
4. A revoked key cannot sign a new bundle (refused at signer side).
5. A bundle signed by a revoked key is refused at verifier side with
   `RevokedKey`.
6. A retired-key bundle within the grace window is accepted; past the
   window it is refused.
7. New catalog tags for rotation/revocation events emit and decode.
8. Trust Report shows trust-anchor state and recent revocations.

## 9. Out of scope

- Cross-fleet trust anchor federation (v0.13 or v0.14).
- Quorum-signed revocation (v0.13 disaster recovery).
- TPM/secure-element-backed revocation roots (deferred with v0.12).
- Live key rotation while a fleet is partitioned (v0.13).
