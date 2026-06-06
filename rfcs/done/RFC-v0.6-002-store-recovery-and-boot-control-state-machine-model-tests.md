# RFC-v0.6-002: Store Recovery and Boot-Control State-Machine Model Tests

**Status.** Implemented (v0.6.0)

## Status

Draft (revised, supersedes pack v0.6-002 draft)

## Target Version

`v0.6.0`.

## Phase

Verification, Fuzzing, and Property Testing — Epic B (Store / Boot Control).

## Related Work

- v0.2 `storaged` append-only log design.
- v0.2 RFC 057 — `bootctl` service extraction.
- v0.3 RFC 003 — `RollbackRecord`.
- v0.4 RFC 004 — `StagingRecord`.
- v0.6 RFC 001 — sibling property-test harness.

---

## 1. Summary

Add a model-test harness for two state machines:

- **storaged recovery** — replay of the append-only log under arbitrary
  truncation and corruption.
- **bootctl** — A/B slot management under arbitrary fault sequences.

The harness uses small explicit models (≤ 200 LOC each) and `proptest` to
explore sequences. Properties focus on *recovery completeness* and
*safety under partial writes*.

---

## 2. Motivation

These two state machines are at the heart of Fjell's reliability promise:

- `storaged` must produce the same "latest authoritative state" regardless
  of which way the log was truncated by an unexpected reboot;
- `bootctl` must never confirm a slot it did not successfully boot, never
  unboot itself, and always allow rollback to the last-known-good slot.

A handful of one-shot QEMU negative tests don't cover sequence-level
properties. Model tests do.

---

## 3. Goals

```text
- One model for storaged log, one model for bootctl.
- 6 properties for storaged, 6 properties for bootctl.
- Sequences include "crash points" (truncation at byte N, where N is
  random).
- Replay must converge to a unique authoritative state.
- bootctl model verified under random fault-injection sequences.
```

## 4. Non-Goals

```text
- No formal verification.
- No actual disk I/O (model only).
- No multi-host store sync (v0.7 scope).
```

---

## 5. External Design

### 5.1 storaged operations modelled

```rust
pub enum StoreOp {
    Append   { kind: RecordKind, payload: Box<[u8]> },
    Crash    { at_byte: u32 },              // truncate the log to at_byte
    Restart,                                // recovery scan over current log
    Compact,                                // future op; v0.6 stubs it
}
```

### 5.2 bootctl operations modelled

```rust
pub enum BootOp {
    SetPending      { slot: Slot, metadata_digest: Digest32 },
    MarkBooted      { slot: Slot },
    ConfirmSlot     { slot: Slot },
    Reboot          { intended_slot: Option<Slot> },
    HealthFail      { slot: Slot },
    PowerLoss       { during_op: BootOpKind },
}
```

---

## 6. Data Model

### 6.1 storaged model state

```rust
pub struct LogModel {
    pub bytes:        Vec<u8>,
    pub committed_records: Vec<CommittedRecord>,
    pub pending_record: Option<CommittedRecord>,
}

pub struct CommittedRecord {
    pub seq:     u64,
    pub kind:    RecordKind,
    pub digest:  Digest32,
    pub byte_offset: u32,
    pub byte_len:    u32,
}
```

### 6.2 bootctl model state

```rust
pub struct BootModel {
    pub slot_a: SlotState,
    pub slot_b: SlotState,
    pub active: Slot,
    pub pending: Option<Slot>,
    pub last_known_good: Slot,
    pub boot_count_since_confirm: u8,
}

pub struct SlotState {
    pub installed:     bool,
    pub metadata_digest: Digest32,
    pub confirmed:     bool,
    pub last_health_ok: bool,
    pub boots:         u32,
}
```

---

## 7. Internal Design

### 7.1 storaged properties

```text
S1: replay_idempotent
    For any log L, two consecutive Restart ops produce the same state.

S2: latest_authoritative_per_kind
    After Restart, for each RecordKind k, the latest committed record of
    kind k is returned by storaged.latest(k).

S3: truncation_drops_partial_records
    For any crash at byte b inside a partially-written record r, after
    Restart, r is absent from the model state. The record before r is
    still present.

S4: digest_failure_skips_record
    A record with a digest mismatch is skipped at replay; the next record
    is processed.

S5: rollback_record_replay_takes_highest
    For any sequence containing RollbackRecord appends (RFC v0.3-003), the
    per-channel min_counter after Restart equals the max counter seen in
    the appends that committed.

S6: staging_record_replay_resumes_or_aborts
    For any sequence of StagingRecord appends (RFC v0.4-004), after
    Restart the resulting staging state is exactly one of:
      (a) the last terminal state appended (Confirmed/Failed/Aborted), or
      (b) the last non-terminal state appended (Fetching/Verifying/...)
    No intermediate states are skipped or invented.
```

### 7.2 bootctl properties

```text
B1: never_confirm_unbooted_slot
    ConfirmSlot succeeds only after a MarkBooted for that slot.

B2: power_loss_during_pending_does_not_promote
    PowerLoss during SetPending leaves the slot in Pending or Unset, never
    Confirmed.

B3: health_fail_rolls_back
    HealthFail on the currently-active newly-booted slot causes Reboot to
    select last_known_good.

B4: boot_count_since_confirm_bounded
    boot_count_since_confirm never exceeds BOOT_COUNT_MAX (=3); on
    overflow, fall back to last_known_good even without explicit health
    fail.

B5: last_known_good_advances_on_confirm
    After a successful ConfirmSlot, last_known_good == that slot.

B6: rollback_does_not_advance_min_counter
    A rollback (BOOT into last_known_good after HealthFail) does not
    modify any RollbackRecord state (cross-checked against the storaged
    model when both are stitched).
```

### 7.3 Combined harness (cross-model)

A composite property `combined_min_counter_invariant`:

```text
For any interleaved sequence of StoreOp and BootOp:
    after the final Restart + Reboot,
    the active slot's metadata_digest's release_counter is >= the
    persisted min_counter for that slot's channel.
```

This stitches together v0.3-003 and v0.4-004 invariants and catches
inconsistency between the two state machines.

---

## 8. Security Design

This is a verification RFC; no runtime change. Discovered violations are
security-relevant because they would otherwise allow:

- a stale state to outlive a crash (S2 breakage);
- a non-confirmed slot to be active after reboot (B1 breakage);
- a partial write to silently corrupt the next read (S3 breakage).

---

## 9. Memory / Resource Design

- Each property runs ≤ 256 ops, ≤ 8 KiB of log bytes; total memory bounded.
- CI runs 1000 cases per property, ~6 properties × 2 models, ~30 s total.

---

## 10. Compatibility and Migration

- No runtime change.
- New CI job step `cargo test -p fjell-store-model -p fjell-bootctl-model`.

---

## 11. Test Strategy

The harness is the strategy. Self-tests:

```text
- log_model_round_trip
- log_model_truncation_to_zero_yields_empty_replay
- boot_model_setup_default_state
- boot_model_force_known_failure_reproduces        (seeded regression)
- combined_min_counter_invariant_self_test
```

---

## 12. Acceptance Criteria

```text
- fjell-store-model + fjell-bootctl-model crates land.
- 6 storaged + 6 bootctl + 1 combined property tested.
- 1000 cases/property green in CI.
- ≥ 4 regression seeds committed.
- ADR-v0.6-002 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/verification/v0.6-002-store-recovery-model.md
docs/src/verification/v0.6-002-bootctl-model.md
docs/src/verification/v0.6-002-properties.md
docs/src/adr/v0.6-002-cross-model-stitching.md
```

---

## 14. Open Questions

1. **Compaction** — v0.6 stubs the compaction op. A future v0.6.x RFC
   defines the compaction semantics and adds properties.
2. **Power-loss granularity** — current model treats power-loss as
   byte-level truncation. Some storage media can corrupt within a sector;
   a v0.6.x RFC may add sector-aware random corruption.

---

## 15. Release Gate (RFC-local)

```text
- Models in CI.
- 13 properties × 1000 cases green.
- Regression files committed.
- ADR Accepted.
```
