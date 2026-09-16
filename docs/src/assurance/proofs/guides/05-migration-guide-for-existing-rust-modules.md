# Migration Guide for Existing Rust Modules

## 1. Purpose

This guide describes how to introduce Verus around an existing Rust module without destabilizing the codebase.

## 2. Migration stages

### Stage 1: Identify invariant

Write the invariant in plain English first.

Example:

```text
cap_mint must never create a child capability with rights outside the parent rights.
```

### Stage 2: Extract pure decision logic

If possible, isolate pure logic into a small Rust helper.

```rust
pub fn rights_subset(child: CapRights, parent: CapRights) -> bool { ... }
```

### Stage 3: Model in Verus

Model the same logic in Verus.

### Stage 4: Add conformance tests

Rust tests should compare behavior against fixed cases or generated vectors.

### Stage 5: Add optional CI target

Run Verus in non-blocking mode first.

### Stage 6: Promote to proof gate only if stable

Proof gate promotion requires reviewer approval.

## 3. Do not rewrite large modules

Do not move a large module into Verus just to “verify it.” Instead, isolate the security-critical core.

## 4. Keep public APIs stable

Verus adoption should not force downstream API churn unless the API was already unsafe or ambiguous.

## 5. Rollback path

Every Verus pilot target should have a rollback plan:

```text
- keep Rust implementation unchanged
- remove CI proof gate if toolchain becomes unstable
- retain conformance tests
- keep proof as documentation if useful
```
