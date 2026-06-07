# RFC-v0.17-005: Verus CI and Proof Gate Policy

**Status:** Proposed
**Milestone:** v0.17
**Derived from:** Verus adoption handoff pack supplement.


## 1. Purpose

This supplement defines how proof checks enter CI without disrupting development productivity.

## 2. Initial proof targets

```text
capability
lease
boot-control
```

## 3. Initial CI status

```text
v0.17.0:
  optional CI, logs uploaded

v0.17.1:
  required for touched target modules

v0.18.0:
  selected targets may become release-required
```

## 4. Required xtask behavior

```text
cargo xtask verus-check <target>
cargo xtask verus-check --all-pilot
cargo xtask verus-check --release-required
```

## 5. Required outputs

```text
VERUS:TARGET:<name>:PASS
VERUS:TARGET:<name>:FAIL
```

## 6. Required documentation

```text
verification/verus/TOOLCHAIN.md
verification/verus/verus-targets.toml
docs/verification/verus/proof-gate-policy.md
```

## 7. Failure isolation

A failure in experimental target must not block unrelated PRs.

A failure in release-required target blocks release.

## 8. Acceptance criteria

```text
- xtask command exists
- at least three pilot targets configured
- optional CI job runs
- release-required target list is explicit
```

## Implementation status (v0.17.0 foundation)

- Proof module: `verification/verus/.../*.rs` (written; pending toolchain pin).
- Conformance test: passing in ordinary `cargo test`.
- Gate level: Experimental (release_required=false).
