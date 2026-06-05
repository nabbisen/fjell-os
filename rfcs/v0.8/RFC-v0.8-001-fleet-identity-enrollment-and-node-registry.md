# RFC-v0.8-001: Fleet Identity, Enrollment, and Node Registry

## Status

Draft (revised, supersedes pack v0.8-001 draft)

## Target Version

`v0.8.0`.

## Phase

Fleet Operations Plane — Epic A (Fleet Identity / Registry).

## Related Work

- v0.7 RFC 001 — NodeIdentity (the building block).
- v0.4 RFC 003 — secure-transportd `FleetEnroll` channel kind.
- v0.8 RFCs 002–005 — consume FleetRoster and node membership.

---

## 1. Summary

Introduce **FleetIdentity**, **EnrollmentRequest/Response**, and
**FleetRoster** — the data model and protocol for a Fjell node to join a
fleet and for fleet members to authenticate each other against a roster
signed by the fleet's root authority.

Fleet identity composes on top of v0.7 node identity: each node has a
node_id (v0.7) **and** a fleet_member_id (v0.8) that maps node_id into a
specific fleet membership context. A node may belong to zero or one fleet
in v0.8.0.

---

## 2. Motivation

The v0.7 pairwise trust model — "same family" or "open" — does not scale
beyond a handful of nodes. Once a fleet exceeds ~10 nodes the operator
needs a single signed roster that every node can reference, with revocation
and member status tracked centrally.

This RFC introduces the minimum primitives: a `FleetRoster` data structure,
an enrollment handshake, and a `fleet-agent` user-space service that
maintains the local membership state.

---

## 3. Goals

```text
- A FleetRoster signed by a FleetRoot key; node_id-keyed membership.
- An EnrollmentRequest / EnrollmentResponse protocol over
  ChannelKind::FleetEnroll (RFC v0.4-003).
- A fleet-agent service that drives enrollment, holds roster, evaluates
  peer identity against roster.
- New KeyPurpose::FleetRoot for the fleet's signing anchor.
- Membership status: Pending | Active | Quarantined | Revoked.
- Roster epoch monotonic; consumers refuse stale rosters.
- All state changes audited.
```

## 4. Non-Goals

```text
- No multi-fleet membership in v0.8.0.
- No fleet root authority discovery — operator provides root pubkey at
  install time (or via signed config bundle).
- No automatic re-enrollment on revoke.
- No cross-fleet trust federation.
- No identity privacy (node_id and alias are visible to fleet root).
```

---

## 5. External Design

### 5.1 Operator workflow

```text
# at the fleet authority (out-of-band):
$ fjell-fleet-tool roster init  --root-key fleet-root.key
$ fjell-fleet-tool roster add   --node-id <hex> --alias edge-001 --status pending
$ fjell-fleet-tool roster sign  --epoch 1  --out roster-v1.bin

# at the enrolling node:
$ fjell-tools fleet enroll --root-pub fleet-root.pub --authority sxt://...
```

The enrolling node ships its `SignedNodeIdentity` to the authority via
`secure-transportd`'s `FleetEnroll` channel; on success it receives the
current roster.

### 5.2 Enrollment handshake

```text
node N → authority A:  EnrollmentRequest {
    node_identity: SignedNodeIdentity,
    nonce:         16 B,
    intended_role: FleetRole,
}

A → N:                  EnrollmentResponse {
    fleet_id:      16 B,
    fleet_member_id: 8 B,
    member_status: MembershipStatus,
    granted_role:  FleetRole,
    roster:        SignedFleetRoster,
    challenge_nonce: 16 B,    // used for next attest push (RFC v0.4-005)
}
```

`EnrollmentResponse` is signed by the FleetRoot key over a domain-separated
digest:

```text
sign_input = SHA256("FJELL-FLEET-ENROLL-V1" || canonical EnrollmentResponse)
```

### 5.3 Roster shape

```rust
pub const FLEET_ROSTER_VERSION: u16 = 1;
pub const MAX_ROSTER_MEMBERS:    usize = 256;

pub struct FleetRoster {
    pub schema_version:  u16,
    pub fleet_id:        [u8; 16],
    pub roster_epoch:    u32,                  // monotonic, signed by root
    pub issued_tick:     u64,
    pub member_count:    u16,
    pub members:         [FleetMember; MAX_ROSTER_MEMBERS],
    pub policy_digest:   Digest32,             // RFC v0.8-005 reference
    pub roster_digest:   Digest32,
}

pub struct FleetMember {
    pub node_id:           NodeId,             // 16 B
    pub fleet_member_id:   [u8; 8],
    pub alias:             [u8; 32],
    pub role:              FleetRole,
    pub status:            MembershipStatus,
    pub since_epoch:       u32,                // epoch at which row was added
    pub attestation_pubkey: [u8; 32],
}

#[repr(u8)]
pub enum FleetRole {
    Member       = 0x01,
    Diagnostic   = 0x02,   // can receive diagnostic pushes
    Recovery     = 0x03,   // can issue recovery intents (RFC v0.8-004)
    Observer     = 0x04,   // read-only
}

#[repr(u8)]
pub enum MembershipStatus {
    Pending     = 0x01,
    Active      = 0x02,
    Quarantined = 0x03,
    Revoked     = 0x04,
}

pub struct SignedFleetRoster {
    pub roster:    FleetRoster,
    pub signature: Signature,
}
```

---

## 6. Data Model

### 6.1 Canonical roster digest

```text
roster_digest = SHA256(
    "FJELL-FLEET-ROSTER-V1" ||
    schema u16 LE || fleet_id 16 B || roster_epoch u32 LE ||
    issued_tick u64 LE || member_count u16 LE ||
    for each member:
        node_id 16 B || fleet_member_id 8 B || alias 32 B ||
        role u8 || status u8 || since_epoch u32 LE ||
        attestation_pubkey 32 B ||
    policy_digest 32 B
)
```

Roster signature uses domain `"FJELL-FLEET-ROSTER-SIGN-V1"`.

### 6.2 New KeyPurpose

```rust
pub enum KeyPurpose {
    // ... existing ...
    FleetRoot = 0x08,    // NEW in v0.8
}
```

`FleetRoot` is an anchor purpose with `AuthorityClass::Genesis` only. The
fleet operator's signing key is the genesis anchor; rotations happen out
of band and produce a roster with a higher epoch.

### 6.3 Persistent membership record

```rust
pub struct MembershipRecord {
    pub schema_version: u16,
    pub fleet_id:       [u8; 16],
    pub member_id:      [u8; 8],
    pub status:         MembershipStatus,
    pub role:           FleetRole,
    pub roster_epoch:   u32,
    pub last_check_tick: u64,
    pub record_digest:  Digest32,
}
```

StoreRecordKind `Membership = 0x18`.

---

## 7. Internal Design

### 7.1 fleet-agent service

`fleet-agent` is a new user-space service that:

- on first boot with a configured fleet-root pubkey: drives enrollment;
- on subsequent boots: loads MembershipRecord + cached roster; refreshes
  roster on request;
- provides IPC API for peer-evaluation:

```text
Tag                      Direction         Payload
FLEET_QUERY              client → fa       _
FLEET_REPLY              fa → client       fleet_id, member_id, status, role
FLEET_EVAL_PEER          client → fa       node_id (16 B), challenge_nonce 16 B
FLEET_EVAL_REPLY         fa → client       allowed u8, role u8, reason u8
FLEET_REFRESH_ROSTER     client → fa       _
FLEET_ROSTER_REPLY       fa → client       roster_epoch, member_count, status
FLEET_LEAVE              client → fa       _
```

### 7.2 Roster verification flow

```text
on receive roster:
  recompute roster_digest; if mismatch → DigestMismatch
  verify signature against anchor (purpose FleetRoot, latest epoch)
  if roster.epoch <= cached.epoch → StaleRoster
  for each member:
      if duplicate node_id within roster → DuplicateMember
      if role not in {Member..Observer} → InvalidRole
  if local node_id present:
      apply own membership state
  else if local node was previously enrolled (cached membership):
      transition to Revoked (own removal)
  persist roster + membership; emit FleetRosterApplied
```

### 7.3 Peer evaluation

```text
on FLEET_EVAL_PEER(peer_id, nonce):
  look up peer_id in roster.members
  if not present → reply { allowed=0, reason=NotInRoster }
  if status != Active → reply { allowed=0, reason=peer.status_as_reason }
  reply { allowed=1, role=peer.role, reason=Ok }
```

Peer evaluation does **not** itself verify a peer's signature on a current
exchange — that's the caller's responsibility. fleet-agent only answers
"would the roster allow this peer at this moment."

### 7.4 Pending → Active transition

A `Pending` member becomes `Active` only via a roster update from the
fleet root. fleet-agent does not self-promote.

---

## 8. Security Design

### 8.1 Threat model deltas

```text
Threat T-200: Adversary publishes a forged roster with attacker-controlled
              members.
Mitigation:  signature verified against pinned FleetRoot anchor;
              cross-protocol replay blocked by domain separator.

Threat T-201: Older roster replayed to re-add a revoked member.
Mitigation:  roster_epoch monotonic; lower-epoch roster rejected.

Threat T-202: Authority loss — fleet root key compromised.
Mitigation:  out-of-band rotation (operator), then roster signed by new
              root; nodes pin first-seen root pubkey but accept a *signed
              succession* envelope (deferred to v0.8.x — for v0.8.0,
              recovery requires re-enrollment via root re-pin).

Threat T-203: Enrollment replay (re-sending old EnrollmentResponse).
Mitigation:  EnrollmentResponse includes a freshness challenge_nonce
              echoed in the next attestation push; older responses fail
              the channel's freshness check.

Threat T-204: Peer claims a role higher than roster grants.
Mitigation:  peer's role is taken from the roster, not from peer's
              self-assertion.

Threat T-205: Fleet root accidentally adds local node with wrong
              attestation_pubkey.
Mitigation:  fleet-agent verifies that the roster member's pubkey matches
              its own SignedNodeIdentity.attestation_pubkey before
              accepting "Active" status for itself; mismatch → Quarantined.
```

### 8.2 Audit emission

```text
FleetEnrollmentInitiated      { fleet_id_first8 }
FleetEnrollmentAccepted       { fleet_id_first8, member_id_first4, role }
FleetEnrollmentRefused        { reason_code }
FleetRosterApplied            { roster_epoch, member_count }
FleetRosterRejected           { error_code }
FleetMemberStatusChanged      { member_id_first4, old, new }
FleetOwnStatusChanged         { old, new }
FleetEvaluatePeer             { peer_id_first8, decision, reason }
```

---

## 9. Memory / Resource Design

- FleetRoster max: 256 × ~120 B + header ≈ 32 KiB. Persisted compressed
  is unnecessary at this size.
- MembershipRecord ≈ 80 B.
- fleet-agent footprint ≈ 36 KiB total.

---

## 10. Compatibility and Migration

- New cap kind `FleetMembership`; new StoreRecordKind 0x18.
- v0.7 pairwise trust still works for unenrolled nodes; once enrolled,
  policy mode `Fleet` (from RFC v0.7-001) is enabled.

---

## 11. Test Strategy

### 11.1 Host unit tests

```text
- roster_digest_covers_members
- roster_digest_covers_policy_digest
- roster_signature_round_trip
- roster_stale_epoch_rejected
- roster_duplicate_member_rejected
- roster_invalid_role_rejected
- enrollment_response_signature_check
- peer_eval_active_returns_role
- peer_eval_revoked_rejected
- peer_eval_unknown_rejected
- own_pubkey_mismatch_quarantines_self
```

### 11.2 QEMU smoke

```text
- SMOKE:FLEET:ENROLL_HAPPY_PATH
- SMOKE:FLEET:ROSTER_REFRESH
- SMOKE:FLEET:EVAL_KNOWN_PEER
```

### 11.3 Negative

| Marker                                              | Profile |
|-----------------------------------------------------|---------|
| `NEG:FLEET:ROSTER_SIGNATURE_FAILED_REJECTED`        | fleet   |
| `NEG:FLEET:STALE_ROSTER_REJECTED`                   | fleet   |
| `NEG:FLEET:DUPLICATE_MEMBER_REJECTED`               | fleet   |
| `NEG:FLEET:UNKNOWN_PEER_REJECTED`                   | fleet   |
| `NEG:FLEET:REVOKED_PEER_REJECTED`                   | fleet   |
| `NEG:FLEET:ENROLL_RESPONSE_REPLAY_REJECTED`         | fleet   |
| `NEG:FLEET:OWN_PUBKEY_MISMATCH_QUARANTINES`         | fleet   |

---

## 12. Acceptance Criteria

```text
- fleet-agent binary exists; fleet-fleet-tool authority CLI exists.
- fjell-fleet-format crate with all types.
- 11 host tests + 3 SMOKE + 7 NEG markers green.
- New KeyPurpose::FleetRoot wired into RFC v0.3-002 keyring.
- Roster persists across reboot.
- ADR-v0.8-001 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/architecture/v0.8-001-fleet-identity.md
docs/src/format/fleet-roster.md
docs/src/operator/fleet-cli.md
docs/src/adr/v0.8-001-fleet-root-purpose.md
docs/src/adr/v0.8-001-single-fleet-membership.md
```

---

## 14. Open Questions

1. **Root rotation** — current design requires re-pin on rotation. v0.8.x
   RFC may add signed-succession envelopes.
2. **Multi-fleet membership** — explicitly out of scope; the
   single-fleet constraint avoids policy ambiguity in v0.8.
3. **Roster compression** — 32 KiB is fine for 256 members; if fleets
   grow past 1k members in v0.9+, introduce delta rosters.

---

## 15. Release Gate (RFC-local)

```text
- fleet-agent + format crate land.
- 11 host + 3 SMOKE + 7 NEG markers green.
- ADRs Accepted.
- CHANGELOG entries filed.
```
