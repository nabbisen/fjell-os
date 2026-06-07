# Verus Toolchain

Verus is **not** a Fjell build dependency. The kernel and all crates build
and test with the stable toolchain in `rust-toolchain.toml`. Verus is an
*optional* proof checker for the pilot targets in `verus-targets.toml`.

## Pinned version

```
verus:  (pin a release tag here, e.g. release/0.2025.xx.xx)
z3:     bundled with the Verus release
```

Record the exact tag when the toolchain is first installed in CI, so proof
results are reproducible. Until then, `cargo xtask verus-check` runs in
conformance-only mode (see below).

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
validated by (a) 19 conformance cases and (b) 13 property tests over the
proved lemmas (`fjell-proptest/tests/verus_lemma_properties.rs`), with a
manual obligation review in
`docs/verification/verus/review-records/v0.17-pilot-targets.md`.
