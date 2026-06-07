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

## Install recipe (Linux x86_64, validated v0.18.x)

```bash
# 1. rustup + the toolchain the Verus binary requires
curl -sL https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init -o /tmp/rustup-init
chmod +x /tmp/rustup-init
/tmp/rustup-init -y --default-toolchain 1.95.0-x86_64-unknown-linux-gnu --profile minimal --no-modify-path

# 2. the pinned Verus release (bundled z3)
TAG="release%2F0.2026.05.24.ecee80a"
ASSET="verus-0.2026.05.24.ecee80a-x86-linux.zip"
curl -sL "https://github.com/verus-lang/verus/releases/download/${TAG}/${ASSET}" -o /tmp/verus.zip
mkdir -p ~/tools/verus && unzip -q /tmp/verus.zip -d ~/tools/verus
chmod +x ~/tools/verus/verus-x86-linux/{verus,rust_verify,z3}

# 3. PATH (shell rc)
export PATH="$HOME/.cargo/bin:$HOME/tools/verus/verus-x86-linux:$PATH"

verus --version   # → 0.2026.05.24.ecee80a / toolchain 1.95.0
```

History: at v0.17.0 the build sandbox could not reach the GitHub release-asset
hosts, so the proofs were temporarily validated by conformance + property
tests only. The hosts are reachable since v0.17.1 and all pilot proofs are
machine-checked (see TOOLCHAIN.lock and the review record).
