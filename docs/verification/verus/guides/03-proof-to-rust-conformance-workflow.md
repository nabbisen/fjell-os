# Proof-to-Rust Conformance Workflow

## 1. Purpose

Verus proves a model. Fjell ships Rust. The bridge between them is conformance.

A proof without conformance is not sufficient.

## 2. Workflow

```text
1. Define the invariant in Verus.
2. Define model transition functions.
3. Prove model invariants.
4. Define conformance cases.
5. Add Rust tests using the same cases.
6. Gate both proof and conformance in CI.
```

## 3. Conformance artifacts

Each target should produce or maintain one of:

```text
- JSON test vectors
- hand-written Rust test cases
- generated Rust test module
- corpus files for fuzz/property tests
```

## 4. Example: boot-control mirror selection

Verus proves:

```text
- valid mirror beats invalid mirror
- higher generation wins
- same generation tie-break is deterministic
```

Rust conformance tests must include:

```text
- A valid, B invalid -> A
- A invalid, B valid -> B
- both valid, A newer -> A
- both valid, B newer -> B
- both valid, same generation -> deterministic tie-break
- both invalid -> NoneValid
```

## 5. Drift detection

A Rust implementation change to a proof-modeled module must update at least one of:

```text
- Verus model
- conformance test
- documentation explaining why proof remains valid
```

If none changes, the reviewer must explicitly confirm that the behavior is unchanged.

## 6. CI rule

For proof-gated targets:

```text
cargo xtask verus-check <target>
cargo test -p <target-crate> <conformance-tests>
```

Both must pass.
