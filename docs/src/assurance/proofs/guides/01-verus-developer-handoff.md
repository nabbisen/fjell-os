# Verus Developer Handoff

## 1. Purpose

This document explains how developers should work with Verus in Fjell OS after the selective adoption decision.

Verus is not a replacement for Rust, QEMU tests, fuzzing, unsafe audits, or security reviews. Verus is a targeted proof tool for small, stable, security-critical logic.

## 2. Developer roles

### Ordinary Rust contributor

Most contributors continue writing normal Rust. They are not required to write proofs unless they modify a proof-gated target.

Expected workflow:

```text
cargo fmt
cargo test
cargo xtask qemu-test ...
cargo xtask qemu-negative ...
```

### Security-boundary maintainer

Maintainers of selected targets update the corresponding Verus model/proof when changing the implementation.

Expected additional workflow:

```text
cargo xtask verus-check <target>
cargo xtask verus-conformance <target>
```

### Reviewer

Reviewers verify that proof changes are justified and remain connected to Rust behavior.

Reviewer focus:

```text
- Does the proof state the real invariant?
- Does the Rust implementation still match the model?
- Did the PR add proof complexity for a low-value target?
- Are conformance tests updated?
```

## 3. Development principle

```text
Proofs must reduce risk, not increase drag.
```

A proof is valuable only if it is:

```text
- small
- reviewed
- connected to Rust behavior
- checked in CI
- scoped to a stable invariant
```

## 4. First pilot targets

The first targets are:

```text
1. capability rights lattice
2. cap_mint non-amplification
3. lease epoch revocation
4. boot-control mirror selection
```

These were chosen because they are small, stable, and central to Fjell's security identity.

## 5. Non-targets for the pilot

The following are intentionally excluded from the first pilot:

```text
- trap entry assembly
- MMIO ordering
- DMA cleanup implementation
- drivers
- scheduler internals
- page table manipulation
- service business logic
- CLI tools
```

These areas may have tests, fuzzing, audits, and QEMU drills, but not initial Verus proof gates.
