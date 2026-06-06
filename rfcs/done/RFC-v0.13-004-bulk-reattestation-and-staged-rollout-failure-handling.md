# RFC-v0.13-004 — Bulk Re-attestation and Staged Rollout Failure Handling

**Status:** Implemented (v0.13.0)
**Target version:** v0.13.0
**Parent:** v0.13-001.
**Cross-refs:** RFC v0.8-003 (staged rollout), v0.11-005 (replay
    cache), v0.13-002 (partition).

## 1. Problem

v0.8 introduced staged rollouts and per-node measurement reports. Two
gaps remain:

- **Bulk re-attestation.** When trust changes (key rotation, threat
  re-evaluation, schema migration), the operator needs the entire
  fleet's current attestation, fresh, within a bounded time. v0.8
  has no procedure for "ask everyone, now, in a controlled way."
- **Staged rollout failure handling.** v0.8 supports staged rollouts
  but the failure semantics are under-specified: what counts as
  failure, who decides, how the rollback propagates.

v0.13 fills both.

## 2. Bulk re-attestation

### 2.1 Why

Triggered by:

- A scheduled compliance window (A3 archetype regulators expect this).
- A trust-spine change (key rotation in flight, schema migration).
- An incident response (a known-good baseline must be re-established).
- A periodic Trust Report refresh.

### 2.2 Controlled re-attest protocol

```text
operator: cargo xtask fleet reattest --window 600s --batch 5

coordinator:
  for each batch of `batch` nodes:
    for each node:
      send AttestRequest { nonce, window_ns: 600_000_000_000 }
    collect responses within window
    record successes / timeouts
    pause `pace_ms` between batches
  emit ReattestManifest summarising the run
```

Key constraints:

- `batch` ≤ configured max (default 16) — protects coordinator and
  link bandwidth.
- `window_ns` is per-request, enforced via v0.11-005 nonce expiry.
- `pace_ms` between batches is configurable; default proportional to
  fleet size.
- Nodes that fail to respond within `window` are *not* automatically
  retried; the manifest lists them, and the operator decides next
  action (quarantine, manual visit, etc.).

### 2.3 Re-attest manifest

A `ReattestManifest` is a signed record committed at the end of a run:

```text
{
  initiated_at_ns:    u64,
  completed_at_ns:    u64,
  trigger_reason:     u8,           — scheduled / rotation / incident / refresh
  fleet_size:         u32,
  attempted:          u32,
  succeeded:          u32,
  timed_out:          u32,
  refused:            u32,
  per_node:           Vec<NodeReattestResult>,
  signature_by:       coordinator key
}
```

The manifest is part of the fleet evidence chain and feeds the next
Trust Report.

### 2.4 Throttle tuning

A small fleet (≤ 32 nodes) re-attests in one batch by default. A
medium fleet (33–256) batches at 8 with 200 ms pacing. Beyond 256 the
operator must explicitly set parameters — at scale the simple defaults
are no longer safe and the policy decision belongs to the operator.

## 3. Staged rollout failure handling

### 3.1 The pipeline (recap from RFC v0.9-004)

```text
Fetched → Verified → Committed → Running → Confirmed | RolledBack
```

`Running → Confirmed` requires a *health check* passing within a
deadline. v0.13 specifies the health check and the failure response.

### 3.2 Health check

A health check is a service-supplied predicate the bundle must satisfy
within `health_check_window_ms` (declared in the `CapManifest`):

- Mandatory marker: the service must emit `BUNDLE_HEALTH:OK` over
  IPC to `service-manager` (a new IPC tag in the existing
  `fjell-service-api::v0_7` namespace).
- The service may emit `BUNDLE_HEALTH:FAILED { reason }` at any time
  during the window to fail early.
- If neither emission arrives before `health_check_window_ms`, the
  rollout is treated as failed (`HealthCheckTimeout`).

### 3.3 Failure response

On failure of a single node:

- The node transitions `Running → RolledBack` per v0.9-004 lifecycle.
- The node emits `BUNDLE.ROLLBACK_TRIGGERED { reason_code }`.
- The coordinator records the failure against the rollout plan.

On a *rate* of failure (fleet-level):

- The rollout plan declares `failure_threshold_pct`. Default: 10%.
- The coordinator monitors per-batch failure rate.
- If exceeded, the rollout pauses; remaining batches are not started.
- The operator receives an actionable summary (catalog intent
  `FLEET.ROLLOUT_PAUSED { failures, total, threshold }`) and decides
  whether to:
  - Resume (overriding the threshold for this rollout).
  - Roll back the already-deployed cohort (`fleet rollout rollback`).
  - Abandon (leave nodes in their current state pending investigation).

### 3.4 Rollback semantics

A rollback is itself a staged operation through the same pipeline. It
is not a magic "revert"; it is a re-rollout of the previous bundle.
The lifecycle states for a rollback rollout are the same; this
preserves auditability — every committed binary on every node has a
provenance chain.

## 4. Partition interaction

If a partition occurs during a rollout (or during re-attestation):

- The partitioned side stops at whatever batch it was on; it does not
  initiate further rollouts.
- Re-attest requests in flight expire by nonce window.
- After reconciliation (v0.13-002), the manifest is re-issued; the
  operator decides whether to resume from the partitioned point or
  restart.

## 5. New / extended catalog intents

- `FLEET.REATTEST_REQUESTED` — coordinator side, per request.
- `FLEET.REATTEST_RESPONDED` — node side, per response.
- `FLEET.REATTEST_MANIFEST_SIGNED` — manifest committed.
- `BUNDLE.HEALTH_OK` — service-emitted from the new bundle.
- `BUNDLE.HEALTH_FAILED` — service-emitted on early failure.
- `BUNDLE.ROLLBACK_TRIGGERED` — node side on failure.
- `FLEET.ROLLOUT_PAUSED` — coordinator side on threshold breach.

All allocated under the reserved fleet/bundle ranges.

## 6. Acceptance criteria

1. `cargo xtask fleet reattest` works end-to-end against the reference
   fleet with documented `batch` and `pace_ms` parameters.
2. `ReattestManifest` is signed, persisted, and surfaces in the Trust
   Report.
3. `BUNDLE_HEALTH:OK` / `BUNDLE_HEALTH:FAILED` IPC tags exist; a
   reference bundle exercises both paths.
4. `failure_threshold_pct` is honoured: a fixture with > threshold
   failures pauses the rollout.
5. Rollback is itself a tracked staged rollout; the audit chain shows
   the full provenance.
6. Partition interaction in §4 is exercised in a fixture.
7. New catalog intents emit and round-trip.
8. Trust Report's "Re-attestation" subsection lists the most recent
   manifest and its summary statistics.

## 7. Out of scope

- Adaptive throttling (machine-learned pacing).
- Multi-region rollouts (multi-fleet topology, post-v1.0).
- Canary cohort selection algorithms beyond "first N in roster
  order"; smarter selection is v1.x.
- Service-side automated rollback choice — the service emits health
  signals; the operator and coordinator decide on rollback.
