# Verus Toolchain

Verus is **not** a Fjell build dependency. The kernel and all crates build
and test with the stable toolchain in `rust-toolchain.toml`. Verus is an
*optional* proof checker for the pilot targets in `verus-targets.toml`.

## Pinned version

```
verus:     release/0.2026.05.24.ecee80a   (x86-linux release asset)
toolchain: 1.95.0-x86_64-unknown-linux-gnu (rustup; required by the Verus binary)
z3:        4.12.5                           (bundled with the Verus release)
```

These are the versions under which the three pilot proofs were first
machine-checked (20 obligations verified, 0 errors as of v0.18.1). The exact pin lives in
`verification/verus/TOOLCHAIN.lock` for reproducibility. Note the Verus
toolchain (1.95.0, via rustup) is independent of the Fjell build toolchain
(`rust-toolchain.toml`, channel 1.91, apt) — Verus is never a Fjell build
dependency.

## Install (developer / CI)

Verus ships as a GitHub release with a bundled rustc and z3:

```bash
# from https://github.com/verus-lang/verus/releases
# unpack, then put the verus binary on PATH
verus --version
```

## Running

```bash
cargo xtask verus-check capability        # one target
cargo xtask verus-check --all-pilot       # all pilot targets
cargo xtask verus-check --release-required # only release-gated targets
```

## Conformance-only mode

If `verus` is not on PATH, `verus-check` does **not** fail the build. It
runs each target's Rust conformance test instead and reports
`VERUS:TARGET:<name>:CONFORMANCE-ONLY`. This matches the Stage A policy:
proofs are additive, never a blocker, until promoted.

## Known environment blocker (v0.17.0)

In the current build sandbox, Verus could not be installed: GitHub
release-asset hosts (`objects.githubusercontent.com`,
`release-assets.githubusercontent.com`) are not in the network allowlist
(`host_not_allowed`), and a source build needs the same hosts plus z3.

Until a network-enabled environment runs the proofs, the pilot targets are
validated by (a) 23 conformance cases and (b) 14 property tests over the
proved lemmas (`fjell-proptest/tests/verus_lemma_properties.rs`), with a
manual obligation review in
`docs/verification/verus/review-records/v0.17-pilot-targets.md`.
