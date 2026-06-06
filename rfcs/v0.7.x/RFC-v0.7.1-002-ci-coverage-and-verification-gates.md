# RFC-v0.7.1-002: CI Coverage and Verification Gate Activation

## Status

Draft (closes review findings **W-RB-02, W-M-05, W-M-06, W-M-07**)

## Target Version

`v0.7.1`

## Summary

The handoff claimed "274 named tests, zero warnings, zero failures" but
the architect's static review found that **41 of 67 workspace packages
are not referenced in CI**, and the promised verification gates
(`proptest`, `unsafe-audit --check`, schema gate, fuzz nightly, ARM64
matrix) are not present in `.github/workflows/ci.yml`.

This RFC ports every workspace package into CI, activates every
verification gate, and adds an ARM64 check matrix.

## Motivation

Whole-project review §4 RB-02:

```text
workspace packages: 67
packages referenced with -p in CI: 26
packages missing from CI package lists: 41
```

The handoff's test posture cannot be trusted as a release gate if the
actual CI does not exercise the relevant crates. This is the second
release blocker on the v0.7.1 path.

Also addressed:

- **W-M-05**: `fjell-arch-arm64` compiles but has no CI matrix; without
  one the stub will bitrot silently.
- **W-M-06**: Fuzz targets exist but no nightly job runs them.
- **W-M-07**: `default-members = []` in the root workspace hides
  coverage gaps when developers run bare `cargo test`.

## Goals

```text
- Every workspace member is either run in CI or explicitly excluded
  with a written reason in workspace metadata.
- proptest, unsafe-audit, schema-gate, and fuzz-nightly jobs are wired
  to .github/workflows/ci.yml.
- ARM64 check matrix runs cargo check on the arch boundary crates.
- A single ci-coverage script enumerates packages and validates that
  CI covers them.
- `cargo test --workspace --lib` (with default-members empty) is
  documented OR default-members is set explicitly.
```

## Non-Goals

```text
- Not adding QEMU ARM64 boot tests (deferred to v0.8 when kernel
  bring-up lands).
- Not turning fuzzing into a per-PR gate (per RFC v0.6-003 §3, fuzz
  is nightly-only).
- Not adding miri matrix yet.
```

## External Design

### CI job matrix (target state)

```yaml
jobs:
  format:        cargo fmt --check
  check:         cargo check --workspace --all-targets
  test-lib:      cargo test --workspace --lib   # see default-members below
  test-formats:  cargo test -p fjell-platform-format -p ... --lib
  test-services: cargo check -p fjell-identityd ... --target riscv64gc-...
  proptest:      cargo test -p fjell-proptest --test harness
  unsafe-audit:  cargo run -p fjell-unsafe-audit -- --root crates --check
  schema-gate:   cargo run -p fjell-tools -- schema verify-frozen
  ci-coverage:   cargo run -p fjell-ci-coverage -- --check
  arm64-check:   cargo check -p fjell-arch-arm64 --target aarch64-unknown-none
  qemu-smoke:    cargo xtask qemu-test m1..m8  (existing)
  qemu-v07:     cargo xtask qemu-test v0.4-net v0.5-platform v0.7-sync
                 (added by RFC-v0.7.1-003)

  fuzz-nightly:
    schedule: '0 4 * * *'
    matrix:   [attestation_v2_parse, release_metadata_parse,
               rollback_record_parse, keyring_snapshot_parse,
               board_profile_parse, update_index_parse,
               diagnostic_bundle_parse, semantic_record_parse]
    runs:     cargo +nightly fuzz run $target -- -max_total_time=300
```

### `fjell-ci-coverage` tool

A new tool at `tools/fjell-ci-coverage/` enumerates workspace members,
parses `.github/workflows/ci.yml`, and confirms each member is either:

1. Referenced by `-p <name>` in at least one CI job, **or**
2. Listed under `[workspace.metadata.fjell.ci_excluded]` in
   `Cargo.toml` with a `reason` field.

Output is a Markdown table written to `docs/src/ci-coverage.md`.

### `default-members`

The architect noted `default-members = []` hides coverage gaps when
developers run `cargo test` without `--workspace`.  Options:

1. Leave as-is and document loudly.
2. Set `default-members` to the lib-test crates.
3. Set `default-members` to the entire host-buildable subset.

**Decision:** option 2 — set `default-members` to the 16 host-testable
library crates explicitly, so that `cargo test` (no flags) runs the
274-test suite.  The fjell-kernel binary remains explicit
(`cargo build -p fjell-kernel ...`) because the cross-target syntax is
unavoidable.

## Data Model

### CI exclusion metadata

In root `Cargo.toml`:

```toml
[workspace.metadata.fjell.ci_excluded]
"fjell-svc-fault"   = { reason = "intentional fault-test target" }
"fjell-svc-timeout" = { reason = "intentional timeout-test target" }
"fjell-neg-test"    = { reason = "negative-test smoke binary; covered by QEMU smoke jobs" }
```

The `fjell-ci-coverage` tool reads this and excludes those packages from
the missing-coverage check.

## Internal Design

### `fjell-ci-coverage` implementation

```text
tools/fjell-ci-coverage/
  src/
    main.rs          CLI entry; --check fails on missing coverage
    workspace.rs     parse Cargo.toml [workspace] members
    ci_yaml.rs       parse .github/workflows/ci.yml for -p references
    report.rs        emit Markdown coverage table
```

The tool does NOT depend on `serde_yaml` (heavy dep); a small
line-based parser handles `cargo test -p` invocations.

### Fuzz nightly job

Per RFC v0.6-003 §7.1, `cargo +nightly fuzz run` with 300-second time
budget per target. Schedule: 0400 UTC daily. Failures upload the
discovered seed to a CI artifact.

### Schema gate

Per RFC v0.6-003 §5.2, regenerate canonical schemas from source and
diff against `crates/*/schema/*.frozen`.  A BREAKING-SCHEMA commit
includes `BREAKING-SCHEMA: <crate>::<format>` in the body to bypass
the gate.

The current `*.frozen` files are checked in but no tool actually
generates schema dumps from source.  This RFC requires the
`fjell-tools schema dump --crate X --format Y` subcommand to be
implemented.  It walks the type definition (parsed via `syn`) and
emits the frozen file format defined in RFC v0.6-003 §6.1.

### ARM64 check matrix

```yaml
arm64-check:
  runs-on: ubuntu-24.04
  steps:
    - run: rustup target add aarch64-unknown-none
    - run: cargo check -p fjell-arch-arm64 --target aarch64-unknown-none
    - run: cargo check -p fjell-arch       --target aarch64-unknown-none
```

`fjell-arch-arm64` is a stub today; the check verifies it stays buildable.

## Security Design

Activating the verification gates IS the security work for v0.7.1-002.
- `unsafe-audit --check` becomes a PR-blocking gate (any unsafe block
  without `// SAFETY:` fails the build).
- `schema-gate` blocks accidental schema drift.
- `proptest` runs 10 properties × 1000 cases on every PR.

## Memory / Resource Design

CI runner budgets:
- `test-lib`: ≈ 90 s on a 4-core runner.
- `proptest`: ≈ 10 s.
- `unsafe-audit`: < 1 s.
- `schema-gate`: < 1 s.
- `fuzz-nightly`: 8 targets × 300 s = 40 min per night.

Total per-PR CI: under 5 minutes target.

## Compatibility and Migration

- Existing PRs may have an uncommented unsafe block. A grace-period
  commit lands the missing SAFETY annotations before this RFC enables
  the gate.  (Per v0.6.0, 261/261 are already covered, so the grace
  period is empty in practice.)
- Schema gate may flag historical drift not previously caught.  All
  drift must be either reverted or accompanied by a BREAKING-SCHEMA
  commit and matching ADR.

## Test Strategy

```text
- Unit tests for fjell-ci-coverage:
    - all members covered → exit 0
    - one member missing → exit 1 with name
    - member listed in ci_excluded → exit 0
- Integration: run fjell-ci-coverage against the actual repo.
- ARM64 check builds without error.
```

## Acceptance Criteria

```text
- fjell-ci-coverage --check exits 0 with all 67 workspace members
  accounted for.
- .github/workflows/ci.yml includes: format, check, test-lib,
  test-formats, test-services, proptest, unsafe-audit, schema-gate,
  ci-coverage, arm64-check, qemu-smoke, qemu-v07, fuzz-nightly.
- fjell-tools schema dump command exists.
- Cargo.toml workspace has default-members listing the 16 lib crates.
- ADR-v0.7.1-002 filed.
```

## Documentation Requirements

```text
- docs/src/ci-coverage.md auto-generated; reviewed each release.
- docs/src/release-process.md updated to list every gate.
- README.md links to docs/src/ci-coverage.md for "how to verify".
```

## Open Questions

```text
1. Should test-services use --target riscv64gc... or also build for
   x86_64 with an HOST_BUILD feature flag? Proposal: RISC-V only for
   v0.7.1; HOST_BUILD path in v0.8 enables host-side service tests.

2. Fuzz nightly findings: who triages? Proposal: CI artifact uploaded
   to a separate branch; weekly review by security WG.

3. Should the unsafe-audit be extended to require categorized SAFETY
   comments (RFC-v0.7.5-001 covers depth)? Proposal: yes, but landing
   in RFC-v0.7.5-001 not here.
```

## Release Gate

A v0.7.1 release tarball MUST have every CI job above passing on the
release commit.  The `fjell-ci-coverage --check` exit code is the
authoritative gate.
