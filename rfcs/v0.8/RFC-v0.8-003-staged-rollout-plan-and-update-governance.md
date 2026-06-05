# RFC-v0.8-003: Staged Rollout Plan and Update Governance

## Status

Draft (revised, supersedes pack v0.8-003 draft)

## Target Version

`v0.8.0`.

## Phase

Fleet Operations Plane — Epic C (Rollout / Governance).

## Related Work

- v0.3 RFC 003 — RollbackRecord & anti-rollback.
- v0.4 RFC 004 — staged-upgrade pipeline.
- v0.7 RFC 002 — snapshot governance pattern.
- v0.8 RFC 001 — FleetRoster.
- v0.8 RFC 002 — FleetView (consumed for cohort visibility).
- v0.8 RFC 005 — fleet policy distribution.

---

## 1. Summary

Introduce **`RolloutPlan`** — a signed, fleet-side description of how a
specific release counter should propagate across the fleet in waves, with
explicit pause/abort gates, cohort assignment, and health thresholds. A
new authority-side service `rolloutd` produces and signs RolloutPlans;
each enrolled `fleet-agent` consumes them and gates local
`upgraded.stage()` accordingly.

The local node update pipeline (v0.4 RFC 004) is reused unchanged; the
rollout plan only controls *when* a node is permitted to stage a given
candidate counter.

---

## 2. Motivation

Without rollout governance, "fleet-wide update" reduces to "every node
fetches simultaneously." That fails for any non-trivial fleet:

- a broken release bricks the entire fleet at once;
- health regression has no built-in pause point;
- coordinated rollback needs explicit cohort membership.

A small signed plan format makes "wave 1 = 10%, observe 24h, wave 2 =
40%, ..." a first-class artifact that every node verifies before staging.

---

## 3. Goals

```text
- RolloutPlan signed by FleetRoot (or delegated RolloutAuthority).
- Cohort assignment deterministic per node (hash(node_id || rollout_id)
  mod cohort_count).
- Wave gates: open_tick, close_tick, min_health_pct, max_failure_pct.
- Pause/Abort: signed amendments raise plan epoch.
- Node refuses staging if no plan or plan epoch ≤ last applied plan epoch
  for the same counter.
- fleet-view tracks per-wave progress.
- All gate decisions audited.
```

## 4. Non-Goals

```text
- No A/B canary by feature flag — the unit of rollout is the release
  counter from RFC v0.3-003.
- No automated rollback orchestration across the fleet. Per-node rollback
  remains a per-node decision (v0.4 RFC 004); v0.8 RFC 004 covers
  fleet-side recovery intents.
- No fine-grained scheduling per wall-clock time. Plans use fleet ticks
  with an external-clock reference (deferred to v0.8.x).
- No multi-channel staggered rollouts in v0.8.0.
```

---

## 5. External Design

### 5.1 Operator workflow

```text
$ fjell-fleet-tool rollout plan \
      --release-counter 64 --channel stable-- \
      --cohorts 5 --wave-fraction 0.05,0.20,0.50,0.80,1.00 \
      --wave-duration-ticks 86400 \
      --min-health-pct 95 --max-failure-pct 1 \
      --out plan-64.bin
$ fjell-fleet-tool rollout sign --key fleet-root.key --out plan-64.sig
$ fjell-fleet-tool rollout distribute --in plan-64.bin --sig plan-64.sig
$ fjell-fleet-tool rollout pause --plan-id <id>
$ fjell-fleet-tool rollout amend --in plan-64.bin --new-epoch 2 --reason "..."
```

### 5.2 Plan shape

```rust
pub const ROLLOUT_PLAN_VERSION: u16 = 1;
pub const MAX_WAVES: usize = 8;

pub struct RolloutPlan {
    pub schema_version:    u16,
    pub plan_id:           [u8; 16],
    pub fleet_id:          [u8; 16],
    pub channel_id:        [u8; 8],
    pub target_counter:    u64,
    pub release_anchor_epoch: u32,           // expected signing epoch
    pub plan_epoch:        u32,              // monotonic per plan_id
    pub state:             PlanState,        // Active | Paused | Aborted
    pub cohort_count:      u8,
    pub wave_count:        u8,
    pub waves:             [Wave; MAX_WAVES],
    pub created_tick:      u64,
    pub plan_digest:       Digest32,
}

pub struct Wave {
    pub wave_index:        u8,
    pub cohort_mask:       u32,              // bits set = included cohorts
    pub open_tick:         u64,
    pub close_tick:        u64,
    pub min_health_pct:    u8,              // 0..=100
    pub max_failure_pct:   u8,              // 0..=100
}

#[repr(u8)]
pub enum PlanState { Active = 1, Paused = 2, Aborted = 3 }

pub struct SignedRolloutPlan {
    pub plan:      RolloutPlan,
    pub signature: Signature,
}
```

### 5.3 Cohort assignment

```text
cohort_index(node_id) = (
    SHA256("FJELL-ROLLOUT-COHORT-V1" || plan_id || node_id)
      .first_u32_le()
  ) mod plan.cohort_count
```

Deterministic; same node + plan → same cohort.

---

## 6. Data Model

### 6.1 Canonical plan digest

```text
plan_digest = SHA256(
    "FJELL-ROLLOUT-V1" ||
    schema u16 LE || plan_id 16 B || fleet_id 16 B ||
    channel_id 8 B || target_counter u64 LE || release_anchor_epoch u32 LE ||
    plan_epoch u32 LE || state u8 ||
    cohort_count u8 || wave_count u8 ||
    for each wave:
        wave_index u8 || cohort_mask u32 LE ||
        open_tick u64 LE || close_tick u64 LE ||
        min_health_pct u8 || max_failure_pct u8 ||
    created_tick u64 LE
)
```

Signing domain: `"FJELL-ROLLOUT-SIGN-V1"`.

### 6.2 Local plan acceptance state

`fleet-agent` persists for each `(fleet_id, plan_id)`:

```rust
pub struct PlanAcceptance {
    pub plan_id:        [u8; 16],
    pub plan_epoch:     u32,
    pub assigned_cohort: u32,
    pub eligible_wave:  u8,         // earliest wave whose mask includes cohort
    pub gate_state:     GateState,
    pub last_check_tick: u64,
    pub record_digest:  Digest32,
}

#[repr(u8)]
pub enum GateState {
    NotReady = 1,        // no wave open yet
    WaveOpen = 2,        // local node may stage now
    PlanPaused = 3,
    PlanAborted = 4,
    HealthBelowFloor = 5, // fleet-view-reported health below min_health_pct
}
```

---

## 7. Internal Design

### 7.1 Plan distribution

`rolloutd` (authority-side) publishes `SignedRolloutPlan` via:

- `secure-transportd` Diagnostics channel pull (each node fetches on
  request), **or**
- embedding in a FleetView snapshot as advisory.

Distribution is *pull* in v0.8.0: nodes ask, authority answers. Push is
a v0.9 feature.

### 7.2 fleet-agent plan evaluation

```text
on tick / on operator request:
  load latest SignedRolloutPlan for own fleet
  verify signature against FleetRoot anchor (current epoch)
  recompute plan_digest; reject mismatch
  if plan.plan_epoch <= cached.plan_epoch (same plan_id): keep cached
  compute cohort_index(self.node_id, plan.plan_id)
  for each wave in order:
      if cohort_mask & (1 << cohort_index):
          if now >= wave.open_tick and now < wave.close_tick:
              gate = WaveOpen
          else if now < wave.open_tick:
              gate = NotReady
          else: continue   # wave closed, move to next
  if no wave matches: gate = NotReady (no eligible wave for cohort yet)
  if plan.state == Paused: gate = PlanPaused
  if plan.state == Aborted: gate = PlanAborted
```

### 7.3 Hooking into upgraded

`upgraded.stage()` (RFC v0.4-004) gains a gating call:

```text
let gate = fleet_agent.evaluate_gate(candidate.counter)?;
match gate {
    WaveOpen => proceed,
    NotReady => Err(GateNotOpen),
    PlanPaused => Err(PlanPaused),
    PlanAborted => Err(PlanAborted),
    HealthBelowFloor => Err(HealthFloorBlocked),
}
```

If the node is *unenrolled* (no fleet), staging is permitted as before
(operator-local control). If the node is enrolled but no plan exists for
the target counter, staging is **refused** with `NoPlan`.

### 7.4 Health floor evaluation

`fleet-agent` pulls a recent `FleetViewSnapshot` (signed) to compute the
percentage of nodes that have successfully confirmed `target_counter`.

```text
health_pct = (confirmed_target / waves[k].cohort_size) * 100
```

If `health_pct < wave.min_health_pct` for the *previous* wave, the
*current* wave is held with `gate = HealthBelowFloor`. This prevents wave
k from opening when wave k-1 is failing.

---

## 8. Security Design

### 8.1 Threat model deltas

```text
Threat T-220: Adversary forges a permissive RolloutPlan to push a bad
              release.
Mitigation:  signature on plan_digest with FleetRoot purpose; nodes
             reject mismatched signature.

Threat T-221: Replay of an older plan_epoch to overrule a Pause amendment.
Mitigation:  plan_epoch monotonic per plan_id; older epoch rejected.

Threat T-222: Plan with mismatched fleet_id imported.
Mitigation:  fleet-agent checks fleet_id matches local membership.

Threat T-223: Cohort mask widened to include all cohorts in one wave
              (effectively no rollout).
Mitigation:  this is a *legitimate* operator choice (emergency push).
              Audited; nothing to prevent — visibility is the mitigation.

Threat T-224: Stale FleetViewSnapshot used to claim health is fine.
Mitigation:  fleet-agent rejects health-floor evaluation with snapshots
             older than MAX_SNAPSHOT_AGE_TICKS (default 1 hour).

Threat T-225: Node bypasses fleet-agent and stages directly.
Mitigation:  cap-broker policy: enrolled nodes' UPGRADE_STAGE right is
             scoped to "with fleet-agent.evaluate_gate". The local
             cap-broker policy bundle (RFC v0.8-005) makes this binding.
```

### 8.2 Audit emission

```text
RolloutPlanAccepted     { plan_id, plan_epoch, cohort }
RolloutPlanRejected     { plan_id, error_code }
RolloutGateEvaluated    { plan_id, counter, gate_state }
RolloutStagingBlocked   { plan_id, counter, gate_state }
RolloutHealthFloorBelow { plan_id, wave, observed_pct, required_pct }
```

---

## 9. Memory / Resource Design

- RolloutPlan ≈ 200 B with up to 8 waves.
- PlanAcceptance ≈ 50 B per cached plan; cap last 4 plans.

---

## 10. Compatibility and Migration

- Pre-enrolled or unenrolled nodes: behaviour unchanged.
- Enrolled nodes that previously had `UPGRADE_STAGE` always granted: the
  v0.8 policy bundle (RFC v0.8-005) tightens this. Operators must roll
  out the policy before enabling enforcement.

---

## 11. Test Strategy

### 11.1 Host unit tests

```text
- plan_digest_covers_waves
- plan_digest_covers_target_counter
- cohort_assignment_deterministic
- cohort_assignment_distribution_balanced     (statistical sample test)
- wave_eligibility_picks_earliest
- plan_epoch_monotonic_required
- plan_paused_blocks_staging
- plan_aborted_blocks_staging
- health_floor_holds_next_wave
- unenrolled_node_unaffected
- enrolled_node_without_plan_refuses_staging
```

### 11.2 QEMU smoke

```text
- SMOKE:ROLLOUT:WAVE_1_STAGES_COHORT_0
- SMOKE:ROLLOUT:PAUSE_BLOCKS_STAGING
- SMOKE:ROLLOUT:AMENDMENT_BUMPS_EPOCH
```

### 11.3 Negative

| Marker                                                  | Profile  |
|---------------------------------------------------------|----------|
| `NEG:ROLLOUT:UNSIGNED_PLAN_REJECTED`                    | rollout  |
| `NEG:ROLLOUT:STALE_EPOCH_REJECTED`                      | rollout  |
| `NEG:ROLLOUT:WRONG_FLEET_REJECTED`                      | rollout  |
| `NEG:ROLLOUT:WAVE_NOT_OPEN_BLOCKS`                      | rollout  |
| `NEG:ROLLOUT:PAUSED_BLOCKS`                             | rollout  |
| `NEG:ROLLOUT:HEALTH_FLOOR_BLOCKS_NEXT_WAVE`             | rollout  |
| `NEG:ROLLOUT:ENROLLED_WITHOUT_PLAN_REFUSES_STAGING`     | rollout  |

---

## 12. Acceptance Criteria

```text
- rolloutd authority service ships.
- fleet-agent gating integrated into upgraded.stage path.
- fjell-rollout-format crate.
- 11 host tests + 3 SMOKE + 7 NEG green.
- Multi-node QEMU SMOKE demonstrates wave behaviour.
- ADR-v0.8-003 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/architecture/v0.8-003-rollout.md
docs/src/format/rollout-plan.md
docs/src/operator/rollout-cli.md
docs/src/adr/v0.8-003-pull-only-distribution.md
docs/src/adr/v0.8-003-cohort-assignment.md
```

---

## 14. Open Questions

1. **External-clock binding** — wave gates use fleet ticks; a future
   v0.8.x RFC adds a signed time anchor so gates can be wall-clock-bound.
2. **Cross-channel rollouts** — out of scope for v0.8.0; one channel per
   plan.
3. **Auto-amend on health-floor breach** — should the authority
   automatically pause? Resolution: no, the human signs the amendment.
   Auto-pause is a v0.9 idea but introduces blast-radius risks.

---

## 15. Release Gate (RFC-local)

```text
- Plan format + rolloutd + fleet-agent gating land together.
- 11 host + 3 SMOKE + 7 NEG markers green.
- ADRs Accepted.
```
