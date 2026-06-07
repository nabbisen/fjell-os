# Verus PR Checklist

Use this checklist when a PR touches Verus-related files or proof-modeled Rust modules.

## General

```text
- [ ] Target is approved for Verus use.
- [ ] Change is not a broad rewrite.
- [ ] Proof scope remains small.
- [ ] Assumptions are documented.
```

## Proof

```text
- [ ] Verus proof passes locally or in CI.
- [ ] Invariant statements are clear.
- [ ] No important behavior is excluded accidentally.
- [ ] Proof does not depend on hidden assumptions.
```

## Rust conformance

```text
- [ ] Rust conformance tests updated.
- [ ] Negative cases included.
- [ ] Production behavior still matches model.
```

## Documentation

```text
- [ ] docs/verification updated.
- [ ] RFC supplement updated if behavior changed.
- [ ] Proof review record added or updated.
```
