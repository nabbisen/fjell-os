# Verus Target Proposal: <Target Name>

## 1. Summary

<Describe the target in one paragraph.>

## 2. Target Tier

```text
Tier 1: model only
Tier 2: Verus-aligned Rust implementation
Tier 3: proof-gated release-critical target
```

## 3. Security Value

```text
What bug class does this proof prevent?
What Fjell invariant does this strengthen?
```

## 4. Rust Modules Affected

```text
crates/...
```

## 5. Invariants

```text
INV-001:
INV-002:
INV-003:
```

## 6. Verus Model Scope

```text
Included:
Excluded:
```

## 7. Conformance Plan

```text
Rust tests:
Generated vectors:
QEMU/negative tests:
```

## 8. CI Plan

```text
Optional check:
Proof gate:
Release gate:
```

## 9. Productivity Risk

```text
Expected proof maintenance cost:
Expected developer impact:
Rollback plan:
```

## 10. Acceptance Criteria

```text
- proof compiles
- Rust conformance tests pass
- documentation exists
- reviewer sign-off complete
```
