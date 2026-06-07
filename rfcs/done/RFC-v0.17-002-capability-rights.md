# RFC-v0.17-002: Verified Capability Rights and Non-Amplification

**Status:** Implemented (v0.17.0; machine-checked v0.17.1)
**Milestone:** v0.17
**Derived from:** Verus adoption handoff pack supplement.


## 1. Purpose

This supplement defines the detailed proof scope for verifying capability rights non-amplification.

## 2. Target Tier

```text
Tier 3: proof-gated release-critical target
```

## 3. Rust modules

Expected modules:

```text
crates/fjell-cap/src/rights.rs
crates/fjell-cap/src/mint.rs
crates/fjell-kernel/src/cap/syscall.rs
```

Exact paths may change after implementation.

## 4. Invariants

```text
CAP-RIGHTS-001: child_rights must be a subset of parent_rights.
CAP-RIGHTS-002: cap_copy does not change rights.
CAP-RIGHTS-003: cap_mint cannot add rights not present in the parent.
CAP-RIGHTS-004: scope cannot be widened by mint.
CAP-RIGHTS-005: dropped or stale handles cannot be used as mint source.
```

## 5. Proof model

The Verus model should include:

```text
Rights bitset
subset predicate
mint_allowed predicate
copy transition
mint transition
```

The first proof does not need to model full CSpace.

## 6. Conformance cases

Rust tests must include:

```text
- equal rights allowed
- strict subset allowed
- adding one extra right rejected
- zero rights allowed only where semantically meaningful
- COPY does not mutate rights
- MINT from stale handle rejected by require_cap path
```

## 7. Out of scope

```text
- lease checks
- object lifetime
- IPC queues
- MMIO/DMA capabilities
```

Those are verified separately.

## 8. Acceptance criteria

```text
- Verus proof passes.
- Rust conformance tests pass.
- cap_mint unit tests reference the same cases.
- proof review record exists.
```

## Implementation status (v0.17.0 foundation)

- Proof module: `verification/verus/.../*.rs` (written; pending toolchain pin).
- Conformance test: passing in ordinary `cargo test`.
- Gate level: Experimental (release_required=false).
