# RFC-v0.17-003: Verified Lease Epoch Revocation

**Status:** Implemented (v0.17.0; machine-checked v0.17.1; C6 retire-before-wrap landed v0.18.1)
**Milestone:** v0.17
**Derived from:** Verus adoption handoff pack supplement.


## 1. Purpose

This supplement defines the detailed proof scope for lease epoch revocation.

## 2. Target Tier

```text
Tier 3: proof-gated release-critical target
```

## 3. Rust modules

Expected modules:

```text
crates/fjell-cap/src/lease.rs
crates/fjell-kernel/src/lease/mod.rs
crates/fjell-kernel/src/cap/syscall.rs
```

## 4. Invariants

```text
LEASE-VERUS-001: A binding is usable only if lease is active and epoch matches.
LEASE-VERUS-002: revoke increments epoch.
LEASE-VERUS-003: a pre-revoke binding is not usable after revoke.
LEASE-VERUS-004: cap_drop remains allowed for revoked capabilities.
LEASE-VERUS-005: safety does not depend on CSpace garbage collection.
```

## 5. Proof model

Minimum model:

```text
Lease { active, epoch }
Binding { epoch_at_issue }
usable(lease, binding)
revoke(lease)
```

Extended model may include:

```text
LeaseId generation
LeaseState Active/Revoked
cap_drop behavior
```

## 6. Conformance tests

```text
- active matching epoch accepted
- active nonmatching epoch rejected
- revoked state rejected
- old binding rejected after revoke
- cap_drop succeeds after revoke
```

## 7. Out of scope

```text
- recursive revocation tree
- cap-broker policy
- blocked IPC wake/cancel
```

Blocked IPC is handled by the later IPC proof target.

## 8. Acceptance criteria

```text
- Verus proof passes.
- Rust lease tests use the same cases.
- v0.2 negative tests remain in CI.
```

## Implementation status (v0.17.0 foundation)

- Proof module: `verification/verus/.../*.rs` (written; pending toolchain pin).
- Conformance test: passing in ordinary `cargo test`.
- Gate level: Experimental (release_required=false).

---

**Amendment (v0.18.1, architect condition C6 — retire-before-wrap).** The
original conformance note documented the kernel's `wrapping_add` on the u32
epoch as a divergence from the unbounded-`nat` model. C6 closes it: the kernel
now routes revocation through `fjell_abi::lease::lease_revoke`, which returns
`RevokeOutcome::MustRetire` at `u32::MAX` instead of wrapping; the Verus model
carries the `epoch < u32::MAX` precondition plus the LEASE-VERUS-005
bounded-domain lemma; and four boundary conformance tests (0, 1, MAX-1, MAX)
pin the behaviour.
