# RFC-v0.8-004: Remote Diagnostics and Recovery Intent

**Status.** Implemented (v0.8.0)

## Status

Draft (revised, supersedes pack v0.8-004 draft)

## Target Version

`v0.8.0`.

## Phase

Fleet Operations Plane — Epic D (Remote Diagnostics & Recovery).

## Related Work

- v0.4 RFC 005 — DiagnosticBundle (the data flowing in).
- v0.8 RFC 001 — FleetRoster (who can issue an intent).
- v0.8 RFC 002 — FleetView.
- v0.8 RFC 005 — fleet policy distribution (parallel mechanism).

---

## 1. Summary

Define **RecoveryIntent** — a signed, bounded directive that a fleet
authority can issue to a specific node (or cohort) to invoke one of a
small, enumerated set of local recovery actions. Define **DiagnosticPull**
— the authority-side counterpart to RFC v0.4-005's diagnostic push: a
signed request that asks a node to assemble and send a fresh bundle.

Both flow through `secure-transportd`. Neither grants arbitrary control;
each is bounded by the action allow-list and the issuing role's rights
from the FleetRoster.

---

## 2. Motivation

When fleet-view shows a node Quarantined or stuck mid-update, the
authority needs a *narrow* way to ask the node to:

- send a fresh diagnostic bundle (already supported via operator-initiated
  push, but slow);
- attempt local rollback to last-known-good;
- enter a recovery state (the node re-attests and waits);
- forget a cached rollout plan and re-pull.

The intent shape encodes "what is allowed" structurally. There is no
"shell" or "raw command" — only enumerated actions.

---

## 3. Goals

```text
- RecoveryIntent: typed action + target + freshness + signature.
- DiagnosticPull: signed request + freshness + response.
- Enumerated action set (5 actions in v0.8.0).
- Authority role gating per action (Member role can't issue intents).
- Local node-side gating: cap-broker policy bundles permit only fleet-
  agent-mediated paths.
- All actions audited and surfaced in semantic stream.
- Intents are idempotent (same intent_id → same result).
```

## 4. Non-Goals

```text
- No remote configuration write — config flows via policy bundles (v0.8
  RFC 005).
- No arbitrary command execution.
- No remote reboot in v0.8.0 (deferred to v0.8.x once reboot policy is
  formalised).
- No interactive sessions.
```

---

## 5. External Design

### 5.1 RecoveryIntent shape

```rust
pub const RECOVERY_INTENT_VERSION: u16 = 1;

pub struct RecoveryIntent {
    pub schema_version:    u16,
    pub intent_id:         [u8; 16],
    pub fleet_id:          [u8; 16],
    pub target_node_id:    NodeId,        // single-node intent in v0.8.0
    pub action:            RecoveryAction,
    pub action_params:     ActionParams,
    pub issued_tick:       u64,
    pub expiry_tick:       u64,
    pub issuer_member_id:  [u8; 8],
    pub challenge_nonce:   [u8; 16],
    pub intent_digest:     Digest32,
}

#[repr(u8)]
pub enum RecoveryAction {
    SendFreshDiagnostic     = 0x01,
    RollbackToLastKnownGood = 0x02,
    EnterRecoveryState      = 0x03,
    ForgetRolloutPlan       = 0x04,
    RefreshRoster           = 0x05,
}

pub struct ActionParams {
    pub plan_id:    [u8; 16],     // valid only for ForgetRolloutPlan
    pub reserved:   [u8; 16],
}

pub struct SignedRecoveryIntent {
    pub intent:    RecoveryIntent,
    pub signature: Signature,
}
```

### 5.2 DiagnosticPull shape

```rust
pub struct DiagnosticPull {
    pub schema_version:    u16,
    pub pull_id:           [u8; 16],
    pub fleet_id:          [u8; 16],
    pub target_node_id:    NodeId,
    pub issued_tick:       u64,
    pub expiry_tick:       u64,
    pub challenge_nonce:   [u8; 16],
    pub pull_digest:       Digest32,
}

pub struct SignedDiagnosticPull {
    pub pull:      DiagnosticPull,
    pub signature: Signature,
}
```

Domain separators:

```text
intent_digest sign input = SHA256("FJELL-RECOVERY-INTENT-V1" || intent_digest)
pull_digest sign input   = SHA256("FJELL-DIAG-PULL-V1" || pull_digest)
```

### 5.3 Role gating

```text
Action                       Required issuer role
SendFreshDiagnostic          Diagnostic | Recovery
RollbackToLastKnownGood      Recovery
EnterRecoveryState           Recovery
ForgetRolloutPlan            Recovery
RefreshRoster                Diagnostic | Recovery
DiagnosticPull               Diagnostic | Recovery
```

A `Member` cannot issue intents at all. An `Observer` cannot issue or
receive — observers only consume FleetView.

---

## 6. Data Model

### 6.1 Canonical intent digest

```text
intent_digest = SHA256(
    "FJELL-RECOVERY-INTENT-V1" ||
    schema u16 LE || intent_id 16 B || fleet_id 16 B ||
    target_node_id 16 B || action u8 ||
    action_params (16 B plan_id || 16 B reserved) ||
    issued_tick u64 LE || expiry_tick u64 LE ||
    issuer_member_id 8 B || challenge_nonce 16 B
)
```

### 6.2 Local execution record

```rust
pub struct IntentExecutionRecord {
    pub schema_version: u16,
    pub intent_id:      [u8; 16],
    pub action:         RecoveryAction,
    pub started_tick:   u64,
    pub completed_tick: u64,
    pub outcome:        IntentOutcome,
    pub record_digest:  Digest32,
}

#[repr(u8)]
pub enum IntentOutcome {
    Completed       = 0x01,
    AlreadyExecuted = 0x02,    // idempotency hit
    Expired         = 0x03,
    UnsupportedAction = 0x04,
    ActionFailed    = 0x05,
}
```

Persisted via storaged kind `IntentExecution = 0x19`. Used both for
audit and for idempotency.

---

## 7. Internal Design

### 7.1 Node-side intent acceptance flow

```text
on SignedRecoveryIntent arrives:
  if intent.fleet_id != self.fleet_id → drop (audit)
  if intent.target_node_id != self.node_id → drop
  if now > intent.expiry_tick → reply Expired
  if intent_id already in IntentExecution log → reply AlreadyExecuted
  fetch issuer member from roster by issuer_member_id
  verify role permits the action; else reply RoleNotPermitted
  verify signature against issuer.attestation_pubkey
  recompute intent_digest; reject mismatch
  dispatch action:
    SendFreshDiagnostic     → diagnosticsd.build_bundle() + push back
    RollbackToLastKnownGood → bootctl.request_rollback_to_lkg()
    EnterRecoveryState      → recoveryd.enter(reason=FleetIntent)
    ForgetRolloutPlan       → fleet-agent.clear_cached_plan(intent.action_params.plan_id)
    RefreshRoster           → fleet-agent.pull_roster()
  on success: append IntentExecutionRecord { Completed }
  audit: RecoveryIntentExecuted { intent_id, action, outcome }
```

### 7.2 DiagnosticPull flow

```text
on SignedDiagnosticPull arrives:
  verify same envelope rules as intent
  diagnosticsd.build_bundle(pull.challenge_nonce)
  attestd.sign_diagnostic(bundle)
  push back over secure-transportd Diagnostics channel
  emit DiagnosticPullCompleted { pull_id, bundle_digest }
```

The bundle that the node pushes back in response binds `pull.challenge_nonce`
into its freshness field (RFC v0.4-005's `nonce_bytes`), so the authority
can verify it answers *this* pull, not a replay.

### 7.3 Authority-side issuance

`recoveryd-authority` is a small authority-host tool that builds and
signs intents. It records every issued intent in a fleet-side log.

### 7.4 Idempotency

`intent_id` is the operator-supplied (or tool-generated) deduplication
key. Re-applying the same intent within its expiry window is a no-op with
outcome `AlreadyExecuted`.

---

## 8. Security Design

### 8.1 Threat model deltas

```text
Threat T-230: Adversary forges a recovery intent to brick a node
              (e.g., RollbackToLastKnownGood loop).
Mitigation:  signature against roster anchor; role gating; expiry_tick;
              idempotency via intent_id.

Threat T-231: Replay of an old intent after the issue was resolved.
Mitigation:  IntentExecution log retains intent_id; replay returns
              AlreadyExecuted without effect.

Threat T-232: Adversary uses DiagnosticPull as a side-channel to
              exfiltrate bytes outside the redaction allow-list.
Mitigation:  bundle contents controlled by diagnosticsd's redaction
              rules (RFC v0.4-005); pull only triggers the build, never
              specifies record kinds.

Threat T-233: Member-role node attempts to issue intents.
Mitigation:  role check at verification; intents from non-permitted
              roles dropped + audited.

Threat T-234: Expired intent re-signed without rotation.
Mitigation:  intent_digest covers expiry_tick; modifying re-signature
              required by adversary, which only the authority can do.

Threat T-235: Cross-fleet intent confusion.
Mitigation:  fleet_id covered by digest; mismatched fleet_id dropped.
```

### 8.2 Audit emission

```text
RecoveryIntentReceived       { intent_id, action, issuer_member_id }
RecoveryIntentRejected       { intent_id, error_code }
RecoveryIntentExecuted       { intent_id, action, outcome }
DiagnosticPullReceived       { pull_id, issuer_member_id }
DiagnosticPullCompleted      { pull_id, bundle_digest }
```

Semantic intents:

```text
0x0152 RECOVERY.INTENT_RECEIVED       (catalog tag)
0x0153 RECOVERY.INTENT_EXECUTED
0x0154 RECOVERY.INTENT_REJECTED
```

These extend the v0.5 RFC 004 catalog under the recovery domain and
require a `BREAKING-SCHEMA`-style audit only if v1 freezes them
out — which the v0.5 RFC explicitly allows for additive intents.

---

## 9. Memory / Resource Design

- SignedRecoveryIntent ≈ 200 B.
- IntentExecution log bounded to 64 most-recent entries; older pruned by
  storaged compaction (future).

---

## 10. Compatibility and Migration

- New StoreRecordKind 0x19.
- New SemanticIntent tags 0x0152-0x0154 (additive).
- New ChannelCap right `SXT_RPC_RECOVERY_INTENT` within Channel cap kind.

---

## 11. Test Strategy

### 11.1 Host unit tests

```text
- intent_digest_covers_action_params
- intent_digest_covers_expiry_tick
- intent_signature_round_trip
- intent_idempotency_same_id_no_effect
- intent_expired_rejected
- intent_role_gating_member_rejected
- intent_wrong_fleet_rejected
- intent_target_mismatch_rejected
- pull_response_binds_challenge_nonce
- pull_signature_failed_rejected
```

### 11.2 QEMU smoke

```text
- SMOKE:RECOV:DIAG_PULL_ROUND_TRIP
- SMOKE:RECOV:ROLLBACK_TO_LKG
- SMOKE:RECOV:FORGET_PLAN_TRIGGERS_REFETCH
```

### 11.3 Negative

| Marker                                                   | Profile  |
|----------------------------------------------------------|----------|
| `NEG:RECOV:UNSIGNED_INTENT_REJECTED`                     | recovery |
| `NEG:RECOV:EXPIRED_INTENT_REJECTED`                      | recovery |
| `NEG:RECOV:WRONG_TARGET_REJECTED`                        | recovery |
| `NEG:RECOV:MEMBER_ROLE_ISSUER_REJECTED`                  | recovery |
| `NEG:RECOV:REPLAY_RETURNS_ALREADY_EXECUTED`              | recovery |
| `NEG:RECOV:DIAG_PULL_NONCE_REPLAY_REJECTED`              | recovery |
| `NEG:RECOV:CROSS_FLEET_REJECTED`                         | recovery |

---

## 12. Acceptance Criteria

```text
- Intent + Pull formats land.
- recoveryd-authority tool exists.
- Node-side recoveryd integrates with bootctl/fleet-agent.
- 10 host tests + 3 SMOKE + 7 NEG markers green.
- New semantic catalog tags accepted.
- ADR-v0.8-004 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/architecture/v0.8-004-remote-recovery.md
docs/src/format/recovery-intent.md
docs/src/format/diagnostic-pull.md
docs/src/operator/recovery-cli.md
docs/src/adr/v0.8-004-action-allow-list.md
docs/src/adr/v0.8-004-role-gating.md
```

---

## 14. Open Questions

1. **Remote reboot** — sufficiently dangerous to defer. A future
   v0.8.x RFC adds `RebootAfterHealthCheck` with strict guard rails.
2. **Multi-node intents** — current shape is single-node. A cohort intent
   (target = cohort_mask) is straightforward but adds blast-radius
   considerations; v0.9 work.
3. **Operator self-service** — should a node operator (local) be able to
   apply intents bypassing the fleet authority? Resolution: yes, via
   local cap-broker paths that don't require signed intents. Both paths
   audited identically.

---

## 15. Release Gate (RFC-local)

```text
- All formats + node-side handler + authority tool land.
- 10 host + 3 SMOKE + 7 NEG markers green.
- ADRs Accepted.
```
