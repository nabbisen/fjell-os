# RFC-v0.17-004: Verified Boot-Control Mirror Selection

**Status:** Proposed
**Milestone:** v0.17
**Derived from:** Verus adoption handoff pack supplement.


## 1. Purpose

This supplement defines the proof scope for deterministic boot-control mirror selection.

## 2. Target Tier

```text
Tier 2 initially, Tier 3 after two stable releases
```

## 3. Rust modules

Expected module:

```text
crates/fjell-upgrade-format/src/boot_control.rs
```

## 4. Invariants

```text
BCB-VERUS-001: valid mirror beats invalid mirror.
BCB-VERUS-002: if both mirrors are valid, higher generation wins.
BCB-VERUS-003: if both mirrors are valid with equal generation, tie-break is deterministic.
BCB-VERUS-004: NoneValid is returned only if both mirrors are invalid.
BCB-VERUS-005: selection is pure and deterministic.
```

## 5. Proof model

Model fields:

```text
valid: bool
generation: nat
```

The first model should not include CRC calculation. It assumes validity has already been established.

## 6. Conformance tests

```text
A valid / B invalid -> A
A invalid / B valid -> B
A valid newer -> A
B valid newer -> B
same generation -> documented tie-break
both invalid -> NoneValid
```

## 7. Out of scope

```text
- CRC computation
- durable write ordering
- storage corruption recovery
```

## 8. Acceptance criteria

```text
- Verus model proves deterministic selection.
- Rust tests cover all matrix cases.
- release notes document tie-break rule.
```

## Implementation status (v0.17.0 foundation)

- Proof module: `verification/verus/.../*.rs` (written; pending toolchain pin).
- Conformance test: passing in ordinary `cargo test`.
- Gate level: Experimental (release_required=false).
