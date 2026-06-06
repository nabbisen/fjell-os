# RFC-v0.11-005 — Replay Cache and Attestation Freshness

**Status:** Proposed
**Target version:** v0.11.0
**Parent:** v0.11-001.
**Cross-refs:** RFC v0.3-004 (attestation profile v2), RFC v0.8-001..005.

## 1. Problem

`AttestationRecordV2` (RFC v0.3-004) is signed and content-addressed,
so a valid record cannot be forged. It *can*, however, be replayed:
the fleet coordinator may receive a previously-valid attestation from
a node that has since been compromised, and accept it as evidence of
current good state.

Replay is the primary remaining attack on the v0.3 trust posture.
v0.11 closes it with a bounded cache and freshness nonces.

## 2. Threat model

Attacker capabilities considered:

- (T1) Network adversary: full read of fleet traffic.
- (T2) Compromise of one node post-attestation.
- (T3) Compromise of a retired signing key (RFC-v0.11-004 already handles
  via revocation; replay handles the gap between compromise and revocation
  distribution).
- (T4) Time-skew: attacker controls a verifier's clock within reasonable
  bounds.

Out of scope for this RFC:

- Compromise of the *current* signing key with no revocation issued
  (covered by v0.11-004 §6 stale-anchor refusal).
- Side-channel timing attacks on signature verification.

## 3. Freshness nonce

Every attestation request carries a 16-byte nonce chosen by the
verifier. The signed `AttestationRecordV2` includes the nonce in its
canonical message domain. The verifier accepts only records whose
nonce matches an outstanding challenge:

```text
verifier sends:    AttestRequest { nonce: [u8; 16], window_ns: u64 }
                      ─────► attesting node
node responds:     SignedAttestationRecordV2 { ..., nonce, ... }
                      ◄─────
verifier checks:   record.nonce ∈ outstanding[node] within window_ns
```

A nonce is single-use. The verifier removes it from `outstanding[node]`
upon successful verification and refuses any later record bearing the
same nonce.

## 4. Replay cache

For interactions where pre-issued nonces are not practical (e.g.
async measurement reports), a bounded sliding-window cache catches
duplicates:

```text
ReplayCache {
    by_record_id: BoundedMap<RecordId, Timestamp>,
    capacity:     usize    = 16_384,
    window_ns:    u64      = 24 hours,
}
```

`RecordId = SHA-256(canonical_attestation_bytes)[0..16]`.

Eviction policy:
- Records older than `window_ns` are evicted on access.
- If the map reaches capacity, oldest 1/8 are evicted in one pass
  (amortised cost, no slow eviction on every insert).

A record presented twice within the window is refused with
`ReplayDetected`. Outside the window, the new submission is accepted;
the practical effect is that the same compromise window cannot be
exploited indefinitely against the same verifier.

## 5. Time handling

Replay defence requires a clock. Fjell's clock is not a wall-clock
guarantee; the verifier uses monotonic nanoseconds since its own
boot. Two consequences:

1. After verifier reboot, the cache is empty. Attestations within the
   pre-reboot window from the *same node* are technically replayable.
   Mitigation: bind the nonce-issuance pattern to the verifier's
   monotonic counter, which is included in the request. A request
   with `verifier_boot_count` lower than current is refused at the
   protocol layer.
2. Clock skew between fleet members is bounded by the request/response
   round-trip and capped at `window_ns`. Beyond that the request
   times out and a fresh nonce is issued.

## 6. Storage and persistence

The cache is **not** persisted. Persisting would convert it from a
soft-state freshness mechanism into a security-critical durable store
with its own integrity story. Boot-time emptiness is acceptable
because:

- Nonce-based challenges (§3) are stateless across verifier reboots
  by construction (the nonce is generated post-boot).
- The replay-cache window (§4) is a defence-in-depth against malformed
  request flows; the nonce path is the primary mechanism.

A future RFC may add a signed persisted log if operational experience
requires it.

## 7. Audit visibility

Three new catalog intents (allocated in the reserved security range):

- `SECURITY.ATTEST_REPLAY_REFUSED` — replay detected; emitted with
  the record_id, source node, and reason.
- `SECURITY.ATTEST_NONCE_EXPIRED` — nonce outside window; emitted on
  late response.
- `SECURITY.ATTEST_NONCE_UNKNOWN` — nonce never issued; suggests
  forgery attempt.

The Trust Report counts these per release window.

## 8. Acceptance criteria

1. `AttestRequest` carries a nonce; `SignedAttestationRecordV2`
   accepts a nonce field in its canonical message.
2. A verifier refuses a record whose nonce is unknown or outside its
   window.
3. A verifier refuses a record whose hash is already in the replay
   cache within the window.
4. The replay cache respects its capacity bound; oldest-eviction is
   tested with a stress fixture pushing 100k records.
5. The three new catalog intents emit on the negative paths and decode
   round-trip.
6. A new QEMU negative-test category `replay` exercises:
   - Replayed valid record refused with `ReplayDetected`.
   - Nonce reused refused with `NonceReplay`.
   - Expired nonce refused with `NonceExpired`.
7. Trust Report's "Replay defence" subsection shows counts per category.

## 9. Out of scope

- Cross-verifier replay coordination (a single attacker hitting
  multiple verifiers with the same record). This is a fleet-wide
  problem; v0.13 addresses it.
- Persistent replay cache (deferred unless operational need arises).
- Time-stamping authority integration (research track).
- Clock-jump protection beyond monotonic counter check.
