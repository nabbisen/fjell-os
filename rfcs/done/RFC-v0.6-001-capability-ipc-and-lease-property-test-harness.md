# RFC-v0.6-001: Capability, IPC, and Lease Property-Test Harness

**Status.** Implemented (v0.6.0)

## Status

Draft (revised, supersedes pack v0.6-001 draft)

## Target Version

`v0.6.0`.

## Phase

Verification, Fuzzing, and Property Testing — Epic A (cap/IPC/lease).

## Related Work

- v0.2 RFCs 035–040 (capability and lease design).
- v0.6 RFC 002 — store / boot-control model tests.
- v0.6 RFC 003 — semantic schema fuzzing.

---

## 1. Summary

Introduce a property-test harness that systematically validates the v0.2
capability/IPC/lease invariants through randomised operation sequences. The
harness uses an in-process model of cap-broker + cap-table + lease-table +
ipc-router and checks oracle properties after each step.

The harness lives in `crates/fjell-proptest`, depends only on host-side
crates (`fjell-cap`, `fjell-ipc`, `fjell-syscall` core, `fjell-tools`),
and runs as `cargo test` with deterministic seeding. CI runs a baseline of
1000 random sequences per property.

---

## 2. Motivation

The v0.2 negative-test suite covers known-bad sequences (one per test). It
does not explore the *combinatorics* between operations: register,
delegate, revoke, replace, fault, lease-expire. With 8 operation types and
sequences up to length 32, the state space is huge but the invariants are
small and well-defined.

Property testing exposes these invariants directly. When a property
violation occurs, the failing sequence is shrunk and recorded as a
regression test (auto-committed under `tests/regressions/`).

---

## 3. Goals

```text
- Single property-test harness exercising cap/IPC/lease.
- 10 named invariants, each tested across ≥ 1000 sequences in CI.
- Shrinking: failed sequences reduced to a minimal failing sequence.
- Regression replay: each failure produces a deterministic seed checked
  in for future runs.
- Deterministic by default; CI uses a fixed seed plus a `RAND_SEED`
  environment override.
- No allocator beyond what proptest already requires on host.
```

## 4. Non-Goals

```text
- No formal verification (TLA+/Coq). Property tests, not proofs.
- No model of the kernel itself; the model is of user-visible cap-broker /
  lease behaviour.
- No multi-thread harness. Single-thread sequences only (Fjell is
  single-hart in v0.6).
- No QEMU integration. Pure host tests.
```

---

## 5. External Design

### 5.1 Test commands

```text
$ cargo test -p fjell-proptest --release          # full CI run (1000/property)
$ cargo test -p fjell-proptest --release -- \
       -- --quick                                  # 100/property (developer loop)
$ RAND_SEED=0xdeadbeef cargo test -p fjell-proptest # replay a specific seed
```

### 5.2 Harness output

On failure:

```text
property: cap_revoke_invalidates_subsequent_use
seed:     0xdeadbeef
minimal failing sequence:
   01: cap_register(cap_id=1, kind=Endpoint, rights=NET_SEND)
   02: cap_delegate(cap_id=1, to=task_2)
   03: cap_revoke(cap_id=1)
   04: ipc_send(task_2, cap_id=1, payload=42)
expected: ipc_send returns LeaseRevoked
actual:   ipc_send returned Ok
```

The sequence is auto-saved to `crates/fjell-proptest/regressions/<hash>.txt`
and added to the always-run test list.

---

## 6. Data Model

### 6.1 Operations

```rust
pub enum Op {
    CapRegister     { kind: CapKind, rights: CapRights },
    CapDelegate     { src_cap: CapId, dst_task: TaskId, sub_rights: CapRights },
    CapRevoke       { cap_id: CapId },
    CapReplace      { cap_id: CapId, new_kind: CapKind, new_rights: CapRights },
    IpcSend         { from: TaskId, cap_id: CapId, tag: u16, payload: u64 },
    IpcRecv         { task: TaskId, endpoint: CapId },
    LeaseExpire     { lease_id: LeaseId },
    LeaseRenew      { lease_id: LeaseId },
    TaskFault       { task: TaskId },
}
```

### 6.2 Model state

```rust
pub struct ModelState {
    pub caps:    BTreeMap<CapId, ModelCap>,
    pub tasks:   BTreeMap<TaskId, ModelTask>,
    pub leases:  BTreeMap<LeaseId, ModelLease>,
    pub now:     u64,
    pub history: Vec<Op>,
}

pub struct ModelCap {
    pub kind: CapKind,
    pub rights: CapRights,
    pub origin_task: TaskId,
    pub lease: LeaseId,
    pub generation: u32,
    pub state: CapState,        // Active | Revoked | Replaced
}
```

---

## 7. Internal Design

### 7.1 Properties (10)

```text
P1: cap_id_never_aliases_after_replace
    For any cap_id and any operation sequence, the (cap_id, generation)
    tuple uniquely identifies one issued cap; after replace, a use of the
    old generation returns StaleGeneration.

P2: revoke_invalidates_subsequent_use
    After cap_revoke(c), every subsequent op that uses c returns LeaseRevoked
    until either:
      - the cap is replaced (generation bumps), or
      - the cap is re-registered as a fresh id.

P3: delegate_subrights_subset_only
    cap_delegate may not grant more rights than the source cap holds.

P4: lease_expiry_revokes_caps_under_it
    When lease L expires, every cap whose .lease == L transitions to Revoked
    in one step.

P5: task_fault_revokes_owned_leases
    When task T faults, every lease originated_by T transitions to Expired
    in one step.

P6: ipc_send_requires_send_right
    ipc_send rejects when cap.rights lacks SEND/IPC_SEND/NET_SEND for the
    cap's kind.

P7: ipc_recv_blocks_or_returns
    ipc_recv on an Endpoint either receives a queued message or returns
    NoMessage; it never panics, never returns stale data from a revoked
    sender.

P8: revoked_cap_inflight_message_dropped
    If task A sends through cap C and the cap is revoked before B's recv,
    the recv either returns LeaseRevoked or NoMessage; it never delivers
    the stale payload.

P9: generation_monotonic
    For any cap_id, the sequence of generations issued is strictly
    increasing.

P10: cap_table_no_capacity_underrun
    Total live caps never exceeds MAX_CAP_TABLE; over-cap registers return
    Error::CapacityExhausted and do not corrupt the table.
```

### 7.2 Generator strategy

`proptest::strategy::Strategy`:

- length: 1 to 64 ops per sequence;
- weighting: register 35%, delegate 15%, revoke 10%, replace 5%,
  ipc_send/recv 20%, lease_expire 5%, lease_renew 5%, task_fault 5%;
- biases: bias `cap_id` argument toward live caps 80% of the time, dead
  caps 20% to ensure negative path coverage.

### 7.3 Shrinking

Standard proptest shrinker:

- drop ops from the end;
- drop ops from the start;
- bisect.

Combined with proptest's regression-file feature, a discovered failure
shrinks to typically 4–8 ops.

### 7.4 Determinism

- Master seed from `RAND_SEED` env or `0x46656C6C` (= "Fell").
- Each property derives a sub-seed `master ^ property_index`.
- Test order independent of the file system: tests iterate over
  `&PROPERTIES` in source order.

---

## 8. Security Design

### 8.1 What this RFC proves

This RFC doesn't introduce a runtime path; it *validates* the boundary
already designed in v0.2. The threats addressed are *design errors*, not
attack vectors:

```text
- Property failure surfaces a v0.2-era invariant break (e.g., generation
  reuse, stale message delivery, lease leakage).
- Regression test pins the discovered defect class.
```

### 8.2 Audit emission

None at runtime. The harness emits structured failure JSON for CI.

---

## 9. Memory / Resource Design

- Each run reserves at most `MAX_CAP_TABLE * sizeof(ModelCap) ≈ 256 KiB`.
- proptest's default 1024 cases × 64 ops ≈ 1 minute per property on a
  modern host.

---

## 10. Compatibility and Migration

- No runtime code changes (this is verification scaffolding).
- A new CI job `cargo test -p fjell-proptest --release` is added.
- A `regressions/` directory in `fjell-proptest` accumulates discovered
  failing seeds.

---

## 11. Test Strategy

The harness *is* the test strategy. The harness itself is unit-tested:

```text
- model_register_then_use
- model_revoke_then_use_rejected
- shrinker_reduces_known_failing_sequence
- regression_files_replay_unchanged
```

---

## 12. Acceptance Criteria

```text
- fjell-proptest crate lands with the 10 properties.
- CI runs the full harness (1000 cases × 10 properties).
- ≥ 4 discovered regression files committed (intentional seeded failures
  pre-fix to prove the harness works).
- Harness completes within 10 minutes on CI baseline hardware.
- ADR-v0.6-001 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/verification/v0.6-001-property-test-harness.md
docs/src/verification/v0.6-001-properties.md       — full list of properties
docs/src/adr/v0.6-001-proptest-as-verification.md
```

---

## 14. Open Questions

1. **Counter-property accumulation** — once 10 properties cover the core
   surface, adding the 11th becomes harder (compounding interactions). A
   v0.6.x RFC can split the harness by domain (caps / ipc / lease).
2. **Coverage measurement** — proptest does not directly measure coverage.
   A future RFC may add coverage-guided fuzzing on top.
3. **Time** — `LeaseExpire` advances `now` by a uniform random amount; a
   real-time-derived expiry mechanism would need separate testing
   (introduced when v0.4 RFC 003 timer surface ships in full).

---

## 15. Release Gate (RFC-local)

```text
- Harness in CI.
- 1000/property runs green on baseline.
- 10 property names, 4+ regression files.
- ADR Accepted.
```
