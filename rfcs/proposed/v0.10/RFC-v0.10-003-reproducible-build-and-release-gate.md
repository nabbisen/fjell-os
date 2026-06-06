# RFC-v0.10-003 — Reproducible Build and Release Gate

**Status:** Proposed
**Target version:** v0.10.0
**Parent:** RFC 061 (release credibility).
**Cross-refs:** RFC-v0.7.1-001 (release metadata), RFC-v0.9-004 (bundle digest).

## 1. Problem

RFC-v0.7.1-001 added release metadata (workspace version,
`rust-toolchain.toml`) but did **not** enforce reproducibility. Two
consecutive builds on the same commit may produce different binaries
because of embedded timestamps, hash-randomised metadata, build-path
leakage, or non-deterministic ordering in `prebuilt/` extraction.

Without reproducibility:

- `bundle_digest` (RFC v0.9-004) is meaningful only for one builder.
- A user cannot verify that the binary they received corresponds to
  the source they audited.
- Trust Report measurements (RFC 061 §6) cannot be cross-checked.

## 2. Sources of non-determinism today

A scan identifies the following risks:

| Source | Fix |
|--------|-----|
| `env!("CARGO_PKG_*")` only — no `chrono::Local` calls anywhere | None needed |
| Build path in panic messages | `--remap-path-prefix` |
| Stable hash of `BTreeMap`/`HashMap` iteration | Use `BTreeMap` only in build outputs |
| Parallel `cargo build` artefact ordering | `cargo build --jobs 1` for release |
| `RUSTC_BOOTSTRAP=1 -Z build-std` cache state | Pinned `rust-toolchain.toml` |
| `prebuilt/*.bin` produced from non-pinned object files | Rebuild from source in `test-all` |
| `target/` left between builds | Clean-room rebuild test |

## 3. Proposed gate

### 3.1 Determinism check

A new tool, `tools/fjell-repro-check/`, runs in CI:

1. Clean checkout of the workspace at HEAD.
2. Build kernel + services to `target/`.
3. Compute SHA-256 of every artefact:
   - `target/.../fjell-kernel`
   - every `crates/fjell-kernel/prebuilt/*.bin`
4. Second clean build (same commit, fresh `target/`).
5. Same artefact digests must match byte-for-byte.

If any digest differs, the gate fails and prints the differing files.

### 3.2 Builder requirements

A reproducible build requires:

- `rustc` exact version from `rust-toolchain.toml`.
- `RUSTFLAGS=--remap-path-prefix=$PWD=.` (added by xtask).
- `SOURCE_DATE_EPOCH` set to the committer date of HEAD (or `0` in
  test mode).
- `--jobs 1` for release builds.
- A documented "reproducible build environment" — a `Dockerfile`
  pinning the host distro is acceptable but not required for v0.10.

### 3.3 What is *not* reproducible

- Test artefacts under `tests/runs/<timestamp>/` — by design, dated.
- Lockfile drift if upstream crates yank versions — pin the lockfile
  by committing `Cargo.lock`.

## 4. Integration with `test-all` and release process

- `cargo xtask test-all` gains a new tier (between `unsafe-audit` and
  `qemu-smoke`) called `repro-check`.
- The release tagger documents one command:

  ```
  cargo xtask release --version vX.Y.Z
  ```

  which runs `test-all`, then `repro-check`, then produces a release
  tarball with digests printed to a `release.txt`.

## 5. Acceptance criteria

1. `tools/fjell-repro-check/` exists and compares two clean builds.
2. `cargo xtask test-all` includes a `repro-check` tier on systems
   where two clean builds fit in disk.
3. Both clean builds of the current commit produce identical
   `fjell-kernel` and identical `prebuilt/*.bin` digests.
4. `cargo xtask release --version vX.Y.Z` exists and produces a
   `release.txt` with digests.
5. `docs/release/reproducibility.md` documents the requirements
   in §3.2.

## 6. Out of scope

- Multi-host reproducibility (Mac vs Linux host). Worthwhile but
  deferred to v0.11.
- Bit-identical between different `rustc` patch levels. The pinned
  `rust-toolchain.toml` is the only supported builder.
- Signing the release tarball. That is the trust-spine RFC-v0.11-x.
