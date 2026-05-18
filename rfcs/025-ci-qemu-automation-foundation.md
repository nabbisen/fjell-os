# RFC 025: CI / QEMU automation foundation

**RFC ID:** 025  
**Also known as:** RFC-v0.1.x-002  
**Status:** Accepted  
**Target version:** v0.1.1  
**Affects:** `.github/workflows/`, `crates/fjell-tools/`

## Problem

v0.2 (Security Boundary Closure) will heavily modify enforcement
behaviour: capability checks, lease revocation, IPC blocking semantics,
MMIO mapping, DMA quarantine, boot-control, and recovery.

Without automated regression tests, changes to any of these will be
fragile and may silently regress security boundaries.

At v0.1.0 the project has the building blocks
(`cargo xtask qemu-test <milestone>`) but no continuous integration,
no negative-test runner, and no convention for capturing or comparing
QEMU serial output as a CI artefact.

## Proposed fix

### CI jobs (GitHub Actions, `.github/workflows/ci.yml`)

| Job | Purpose |
|---|---|
| `ci-format` | `cargo fmt --check` |
| `ci-check`  | `cargo check --workspace` (host-buildable members only) |
| `ci-test-host` | `cargo test` on all `*-format` crates and `fjell-tools` |
| `ci-qemu-smoke` | `cargo xtask qemu-test m1..m8` |
| `ci-qemu-negative` | `cargo xtask qemu-negative <category>` for the categories that exist |

A failed `TEST:Mx:PASS` or a missing expected `NEG:*:PASS` marker must
fail the relevant job.

### `xtask` surface (`crates/fjell-tools`)

The tool gains, in addition to existing `qemu-test`:

```
cargo xtask qemu-test <milestone>              # already exists
cargo xtask qemu-negative <category>           # new — RFC 026
cargo xtask qemu-log-check <log-file> <marker> # new — generic checker
cargo xtask qemu-run --profile <profile>       # new — configurable run
```

`qemu-log-check` is a pure pattern-match against a captured serial log
and is used by every CI job to assert markers.  It is a small generic
helper, not a milestone-specific runner.

`qemu-run --profile <profile>` reads a profile file under
`tests/qemu/profiles/<profile>.toml` describing the QEMU invocation,
expected markers, timeout, and disk-image setup.  Smoke and negative
runners are thin wrappers around it.

### Captured artefacts

Each QEMU CI job must upload, under
`tests/qemu/artifacts/<run-id>/`:

- `serial.log` — QEMU `-nographic` combined stdout/stderr
- `qemu-command.txt` — exact command line used
- `expected-markers.txt` — list of `TEST:*:PASS` / `NEG:*:PASS` markers
- `result-summary.txt` — overall pass / fail

Failing runs always upload these so the failure is reproducible from
the artefact alone.

## Rationale

- Centralising the QEMU run in a `--profile`-driven helper makes the
  smoke and negative paths share the same code, avoiding two parallel
  evolutions.
- Capturing artefacts is the difference between *“CI told me it broke”*
  and *“CI told me it broke and here is exactly what the serial port
  said”*.  Without the second, post-mortem after CI failure becomes a
  manual rerun on a developer machine.
- Choosing GitHub Actions matches the existing badge in `README.md`
  (`actions/workflows/ci.yml`); no new vendor surface is added.

## Impact

- New crate `fjell-tools` subcommands: behind the existing `cargo xtask`
  entry point, no kernel impact.
- New directory `tests/qemu/profiles/` and `tests/qemu/artifacts/`.
- Workflow file under `.github/workflows/`.
- Backward compatibility: full; `cargo xtask qemu-test m4` continues to
  work as before.

## Test plan

- Local: `cargo xtask qemu-log-check tests/qemu/fixtures/m4-pass.log
  TEST:M4:PASS` must exit 0.
- Local: same command with a non-existent marker must exit non-zero.
- CI: the workflow file passes `actionlint`.
- CI: on a known-good commit, every smoke job emits its `TEST:Mx:PASS`
  marker.
- CI: when an artificial test failure is injected
  (`expected-markers.txt` lists a marker that is not produced), the
  job fails and uploads its serial log.

## Implementation notes

- Out of scope: hardware CI, performance benchmark gating, formal
  verification gate, fuzzing gate.  All belong to later versions.
- Negative test categories defined here are container labels only; the
  individual negative tests are added by RFC 026.
- The `cargo xtask qemu-negative <category>` subcommand must succeed
  with a no-op message when no negative tests for `<category>` are
  registered yet, so that CI can be wired before RFC 026 lands all
  categories.  This avoids a chicken-and-egg deadlock between CI and
  tests.
