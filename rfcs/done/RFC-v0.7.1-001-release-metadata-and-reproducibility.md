# RFC-v0.7.1-001: Release Metadata and Reproducibility

**Status.** Implemented (v0.7.1)

## Status

Draft (closes review finding **W-RB-06**)

## Target Version

`v0.7.1`

## Summary

Make v0.7.x release artifacts fully reproducible: consistent version
metadata across `README.md`, `Cargo.toml`, `CHANGELOG.md`, and release
notes; pin the Rust toolchain via `rust-toolchain.toml`; commit
`Cargo.lock`; ship a `RELEASE.md` per tarball that captures the exact
commands used to generate the headline test counts.

## Motivation

The whole-project review (§4, RB-06) noted:

- `Cargo.toml` says `0.7.0`, but `README.md` still says
  `Current version: v0.3.0-alpha.1`.
- The release archive excludes `Cargo.lock`, while the workspace uses
  external dependencies (`proptest`, `libfuzzer-sys`).
- No `rust-toolchain.toml` was found.

A v0.7 review and release must be reproducible. Stale metadata and an
unpinned toolchain prevent independent verification of the 274-test
posture claimed in the handoff.

## Goals

```text
- README.md, Cargo.toml, CHANGELOG.md, and release notes agree on version.
- rust-toolchain.toml pins the reviewed toolchain to a specific stable.
- Cargo.lock is committed and shipped with the release tarball.
- RELEASE.md documents the exact commands used for the headline counts.
- Release tarball includes a manifest of file digests.
```

## Non-Goals

```text
- No change to release cadence.
- No automated signing of release tarballs (planned for v0.8).
- No change to dependency versions; lock file captures current state.
```

## External Design

### Versioning policy

- `Cargo.toml` `version` is the single source of truth.
- `README.md`, `CHANGELOG.md`, `docs/src/SUMMARY.md`, and per-release
  `RELEASE.md` reference that version explicitly with `vM.m.p` form.
- CI gate `ci-version-consistency` greps for stale `v0.X` references
  outside `CHANGELOG.md` (which contains historical references by
  design).

### Toolchain pinning

A new file `rust-toolchain.toml` at workspace root:

```toml
[toolchain]
channel = "1.91"
profile = "minimal"
components = ["rustfmt", "clippy", "rust-src"]
targets   = ["riscv64gc-unknown-none-elf"]
```

For nightly-only tasks (fuzzing), `cargo +nightly` continues to be
invoked explicitly; the pinned toolchain file does not block ad-hoc
nightly invocations.

### Lockfile policy

For a workspace that produces a kernel image, library API surface is
relevant but reproducibility of host-side test builds is critical.
Policy: **`Cargo.lock` is committed and shipped.**

This is a change from the v0.4–v0.7 tarball-exclusion policy.  Rationale:
the review noted that fuzz and proptest dependencies bring in transitive
graphs that may shift between checkouts.

### RELEASE.md per release

Each tarball includes a `RELEASE.md` at the root containing:

```text
- exact git commit (or "unreleased") and tag
- exact Rust channel and toolchain components used
- the precise cargo invocations that produced the headline counts
- SHA-256 of every prebuilt .bin under crates/fjell-kernel/prebuilt/
- known-broken items quoted from CHANGELOG
```

## Data Model

No new types.

## Internal Design

### Implementation steps

1. Write `rust-toolchain.toml` at workspace root.
2. Commit `Cargo.lock`.
3. Update `release.policy.tarball.exclude` (in xtask source) to no
   longer exclude `Cargo.lock`.
4. Update `README.md` to reflect v0.7.x state; move per-version
   narrative into `CHANGELOG.md` and `docs/src/`.
5. Write a `tools/fjell-release/` xtask that:
   - reads `Cargo.toml` version,
   - greps for stale version mentions outside `CHANGELOG.md`,
   - generates `RELEASE.md` with the headline command and counts,
   - generates a file-digest manifest of `crates/fjell-kernel/prebuilt/`,
   - exits non-zero on any inconsistency.

### `ci-version-consistency` gate

```bash
cargo run -p fjell-release -- --check
```

Fails CI on any of:

```text
- README.md still references a version other than Cargo.toml version
- RELEASE.md present but headline command no longer in xtask
- Cargo.lock absent from a non-development build
```

## Security Design

Reproducibility is the security property here.  A signed release process
is the v0.8 deliverable; v0.7.1 lays the groundwork by removing
ambiguity about *which bits* are in any given release.

## Memory / Resource Design

None — pure release-engineering change.

## Compatibility and Migration

- Downstream consumers using the v0.7.0 tarball: must regenerate their
  builds against the committed `Cargo.lock`.  This is a recommended
  upgrade, not breaking.
- The `Cargo.lock` policy change requires updating any downstream CI
  that excluded the file.

## Test Strategy

```text
- Unit tests for fjell-release version-grep logic.
- CI smoke: ci-version-consistency runs on every PR.
- Manual verification: a fresh checkout with the pinned toolchain
  reproduces the 274-test posture exactly.
```

## Acceptance Criteria

```text
- README.md, Cargo.toml, CHANGELOG.md, docs/src/SUMMARY.md agree on
  the current version.
- rust-toolchain.toml exists, pins channel = "1.91" (or current).
- Cargo.lock is committed.
- v0.7.1 tarball includes Cargo.lock, rust-toolchain.toml, RELEASE.md.
- ci-version-consistency job is green.
- Independent reviewer can reproduce 274 tests with one shell command
  from a fresh checkout.
```

## Documentation Requirements

```text
- README.md updated to v0.7.1.
- docs/src/release-process.md created (or updated) to describe the
  RELEASE.md generation flow.
- CHANGELOG.md gains a v0.7.1 entry citing this RFC.
- ADR-v0.7.1-001 filed referencing this RFC's outcome.
```

## Open Questions

```text
1. Does the kernel binary itself need a build-info section embedded?
   Proposal: defer to v0.8 signed-release work.
2. Should we publish SBOM (Software Bill of Materials) format? CycloneDX
   vs SPDX. Proposal: cargo-cyclonedx in v0.8.
```

## Release Gate

This RFC is itself the release gate for any v0.7.x patch tarball.
A v0.7.1+ tarball that fails `ci-version-consistency` is not a release.
