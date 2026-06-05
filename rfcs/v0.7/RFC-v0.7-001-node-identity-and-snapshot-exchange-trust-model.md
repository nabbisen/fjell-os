# RFC-v0.7-001: Node Identity and Snapshot Exchange Trust Model

## Status

Draft (revised, supersedes pack v0.7-001 draft)

## Target Version

`v0.7.0`.

## Phase

Distributed Snapshot Sync — Epic A (Node Identity).

## Related Work

- v0.3 RFC 001 — `HardwareTrustProvider` (origin of node identity).
- v0.3 RFC 002 — keyring (`KeyPurpose::SnapshotSigning` reserved).
- v0.4 RFC 003 — `secure-transportd` (transport for exchange).
- v0.7 RFCs 002/003/004 — consumers.

---

## 1. Summary

Define **NodeIdentity**: a per-device identity record bound to the trust
provider, signed by the device's `KeyPurpose::AttestationSigning` anchor,
and stable across reboots. Define the *trust model* for snapshot
exchange between two Fjell nodes: how each node verifies the other, what
data crosses, and where the policy boundary lives.

This RFC stands on its own; subsequent RFCs (v0.7-002 export/import, v0.7-003
audit policy, v0.7-004 conflict-domain metadata) build on top.

---

## 2. Motivation

v0.4 introduced server-side trust (pinned anchors for known servers).
Node-to-node exchange requires *bidirectional* identity:

- Alice must know that Bob is a genuine Fjell node and not an impostor.
- Bob must verify the same about Alice.

A new `NodeIdentity` record provides the wire-shape; the same trust-provider
+ keyring infrastructure provides the signing.

---

## 3. Goals

```text
- One canonical NodeIdentity record per device.
- Signed by the device's attestation-signing key.
- Carries TrustProviderId and trust profile.
- Carries identifier-stable across reboots: derived from sealed seed.
- Optional pseudonymous alias separate from the cryptographic identity.
- A pairwise trust-decision rule documented and tested.
```

## 4. Non-Goals

```text
- No PKI; identity is direct-pinning, like v0.4 RFC 003 server anchors.
- No identity *issuance* by Fjell itself in v0.7; identity is asserted by
  the device, accepted by peers per policy.
- No persistence beyond storaged (no external "identity service").
- No central directory.
```

---

## 5. External Design

### 5.1 NodeIdentity record

```rust
pub const NODE_IDENTITY_VERSION: u16 = 1;

pub struct NodeIdentity {
    pub schema_version:        u16,
    pub node_id:               NodeId,             // 16 B, derived from seed
    pub alias:                 [u8; 32],           // user-set human-readable label
    pub created_tick:          u64,
    pub trust_provider_id:     TrustProviderId,
    pub trust_profile_tag:     u8,
    pub attestation_pubkey:    [u8; 32],          // Ed25519 pubkey
    pub platform_digest:       Digest32,
    pub board_digest:          Digest32,
    pub identity_digest:       Digest32,
}

pub struct SignedNodeIdentity {
    pub identity:      NodeIdentity,
    pub signature:     Signature,
    pub signed_at_epoch: u32,                     // keyring epoch
}
```

### 5.2 NodeId derivation

```text
seed = sealed_data_key for purpose SealedDataKey
       (unsealed at boot through HardwareTrustProvider)
node_id = SHA256("FJELL-NODE-ID-V1" || seed || platform_digest || board_digest)
          truncated to 16 B
```

The seed is stable across reboots (sealed key persists). If the platform
or board profile changes, the node_id changes — this is by design: a
re-imaged device is a new node.

### 5.3 Pairwise trust decision

When node A receives a `SignedNodeIdentity` from node B, A's policy decides:

```text
1. Verify signature against B's attestation_pubkey (carried in identity).
2. Cross-check identity_digest matches the recomputed value.
3. Check trust_profile_tag against A's policy allow-list.
4. Check (platform_digest, board_digest) against A's policy:
     - "same family" mode: must match A's own pair, or
     - "fleet" mode: must appear in a signed roster, or
     - "open" mode: no profile constraint (development only).
5. Check node_id pin (if A has a pinned roster).
```

Policy bundles loaded by `verifyd` (via cap-broker) encode the rule. v0.7
ships the "same-family" mode as default; "fleet" mode lands with v0.8.

---

## 6. Data Model

### 6.1 Canonical identity_digest

```text
identity_digest = SHA256(
    "FJELL-NODE-ID-V1" ||
    schema u16 LE ||
    node_id 16 B ||
    alias 32 B ||
    created_tick u64 LE ||
    trust_provider_id u32 LE ||
    trust_profile_tag u8 ||
    attestation_pubkey 32 B ||
    platform_digest 32 B ||
    board_digest 32 B
)
```

### 6.2 Policy structure (declarative)

```rust
pub struct NodeIdentityPolicy {
    pub mode:               TrustMode,         // SameFamily | Fleet | Open
    pub allowed_profiles:   [u8; 4],           // trust_profile_tag whitelist
    pub allowed_count:      u8,
    pub pinned_roster:      Option<RosterRef>, // v0.8 forward-compat
    pub policy_digest:      Digest32,
}

#[repr(u8)]
pub enum TrustMode { SameFamily = 1, Fleet = 2, Open = 3 }
```

### 6.3 Storage

`SignedNodeIdentity` is persisted by storaged under record kind
`StoreRecordKind::NodeIdentity = 0x15`. Recovery scan picks the latest;
the prior identity remains in the log for audit.

---

## 7. Internal Design

### 7.1 Identity service: `identityd`

A small service that:

- on first boot: derives node_id, builds NodeIdentity, requests signature
  from attestd, persists SignedNodeIdentity;
- on subsequent boots: loads the persisted record, verifies digest, makes
  it available via IPC.

`identityd` holds no signing key; it borrows attestd for the actual
signature.

### 7.2 IPC surface

```text
Tag                       Direction      Payload
IDENT_QUERY               client → idd   _
IDENT_REPLY               idd → client   node_id, alias, profile, epoch
IDENT_EXPORT_SIGNED       client → idd   _
IDENT_EXPORT_REPLY        idd → client   bytes of SignedNodeIdentity, len
IDENT_EVALUATE_PEER       client → idd   bytes of SignedNodeIdentity
IDENT_EVALUATE_REPLY      idd → client   decision u8, reason u8
```

### 7.3 Alias update

Operator-supplied via `fjell-tools identity set-alias "..."`. Updating the
alias re-signs the identity but does *not* change node_id. The audit
record captures the prior alias for traceability.

---

## 8. Security Design

### 8.1 Threat model deltas

```text
Threat T-160: Adversary spoofs a peer identity with a matching node_id.
Mitigation:  signature over identity_digest with the peer's
             attestation_pubkey; pubkey is part of the digest.

Threat T-161: Compromised node leaks its sealed seed.
Mitigation:  seed is sealed with the trust provider; a software provider
             leak is a known soft mitigation (called out in v0.3 RFC 001
             as a residual until hardware lands). The node_id is replaced
             on next reseal.

Threat T-162: Alias used as security identifier.
Mitigation:  identity_digest covers alias but policy never keys off alias;
             the trust decision uses node_id and pubkey only.

Threat T-163: Replay of a stale SignedNodeIdentity.
Mitigation:  pairwise exchange wraps the identity inside a freshness
             envelope (handshake nonce); v0.7 RFC 002 defines this.

Threat T-164: Profile-tag confusion (TPM profile pretending to be DICE).
Mitigation:  trust_profile_tag is part of identity_digest; the verifier
             policy enumerates allowed tags and rejects unlisted ones.
```

### 8.2 Audit emission

```text
NodeIdentityIssued           { node_id_first8, alias_first8, profile_tag }
NodeIdentityAliasChanged     { old_alias_first8, new_alias_first8 }
NodeIdentityVerifyFailed     { peer_node_id_first8, error_code }
NodeIdentityEvaluatePeer     { peer_node_id_first8, decision, reason_code }
```

---

## 9. Memory / Resource Design

- NodeIdentity ≈ 200 B; SignedNodeIdentity ≈ 270 B.
- identityd cache: 1 own + last 4 peers = 5 records ≈ 1.4 KiB.

---

## 10. Compatibility and Migration

- New StoreRecordKind reserved (0x15).
- No prior identity records exist; migration is empty.
- attestd gains a `sign_identity` typed call that re-uses
  AttestationSigning purpose with a domain-separator
  ("FJELL-NODE-ID-V1") to prevent cross-protocol replay.

---

## 11. Test Strategy

### 11.1 Host unit tests

```text
- node_id_deterministic_for_fixed_seed
- identity_digest_covers_alias
- identity_digest_covers_platform_board_digests
- signed_identity_round_trip
- alias_change_does_not_alter_node_id
- evaluate_peer_same_family_accepts_matching
- evaluate_peer_same_family_rejects_different_board
- evaluate_peer_unknown_profile_rejects
- evaluate_peer_bad_signature_rejects
```

### 11.2 QEMU smoke

```text
- SMOKE:IDENTITY:FIRST_BOOT_PERSISTED
- SMOKE:IDENTITY:RELOAD_AFTER_REBOOT
- SMOKE:IDENTITY:EVALUATE_SAMPLE_PEER
```

### 11.3 Negative

| Marker                                                  | Profile  |
|---------------------------------------------------------|----------|
| `NEG:IDENT:SIGNATURE_FAILED_REJECTED`                   | identity |
| `NEG:IDENT:DIFFERENT_BOARD_REJECTED`                    | identity |
| `NEG:IDENT:UNKNOWN_PROFILE_REJECTED`                    | identity |
| `NEG:IDENT:STALE_REPLAY_REJECTED`                       | identity |
| `NEG:IDENT:ALIAS_CHANGE_PRESERVES_NODE_ID`              | identity |

---

## 12. Acceptance Criteria

```text
- identityd binary exists.
- fjell-identity-format crate with NodeIdentity types.
- attestd gains sign_identity path.
- 9 host tests, 3 SMOKE, 5 NEG markers green.
- New StoreRecordKind committed.
- ADR-v0.7-001 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/architecture/v0.7-001-node-identity.md
docs/src/format/node-identity.md
docs/src/adr/v0.7-001-pairwise-trust-model.md
docs/src/adr/v0.7-001-node-id-derivation.md
```

---

## 14. Open Questions

1. **node_id collision** — birthday-bound at 2^64 for SHA-truncated 16 B.
   Acceptable for any plausible Fjell fleet size.
2. **Alias uniqueness** — not enforced; v0.8 fleet roster may impose
   uniqueness as a fleet-policy concern.
3. **Identity rotation** — only via re-sealing the seed (factory reset
   semantics). Voluntary rotation in-place is intentionally out of scope.

---

## 15. Release Gate (RFC-local)

```text
- identityd ships.
- All tests green.
- ADRs Accepted.
- CHANGELOG entries filed.
```
