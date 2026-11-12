# Verus Setup

The complete Verus installation recipe, pinned version, and background
are documented in
[`verification/verus/TOOLCHAIN.md`](https://github.com/nabbisen/fjell-os/blob/main/verification/verus/TOOLCHAIN.md)
at the repository root.

## Quick reference

Verus is **optional** — the kernel and all crates build without it.
It is only needed for Gate 10 (`release-rehearsal`) and explicit
`cargo xtask verus-check` runs.

### Pinned version

**This page does not carry the pinned versions.** They live in
[`verification/verus/TOOLCHAIN.lock`](https://github.com/nabbisen/fjell-os/blob/main/verification/verus/TOOLCHAIN.lock),
with the install route and the upgrade procedure in
[`TOOLCHAIN.md`](https://github.com/nabbisen/fjell-os/blob/main/verification/verus/TOOLCHAIN.md)
beside it — and `cargo xtask verus-check` refuses to run against anything else.

A copy here is a second place for the same three figures to go stale, which is
what **E-109** was filed for on another page and what **E-119** was filed for on
this one: the block this paragraph replaces named a prover two pins old, a z3 two
upgrades old, and "Fjell's 1.91" long after the channel was 1.98.1.

### Linux x86\_64 install

```sh
# 1. Rustup toolchain for Verus
rustup toolchain install 1.95.0-x86_64-unknown-linux-gnu --profile minimal

# 2. Download pinned Verus release
TAG="release%2F0.2026.05.24.ecee80a"
ASSET="verus-0.2026.05.24.ecee80a-x86-linux.zip"
curl -sL "https://github.com/verus-lang/verus/releases/download/${TAG}/${ASSET}" \
  -o /tmp/verus.zip
mkdir -p ~/tools/verus && unzip -q /tmp/verus.zip -d ~/tools/verus
chmod +x ~/tools/verus/verus-x86-linux/{verus,rust_verify,z3}

# 3. Add to PATH
export PATH="$HOME/.cargo/bin:$HOME/tools/verus/verus-x86-linux:$PATH"

# 4. Verify
verus --version   # → 0.2026.05.24.ecee80a
```

### Run

```sh
cargo xtask verus-check --all-pilot         # all three pilot targets
cargo xtask verus-check --release-required  # capability + lease only (Gate 10)
cargo xtask verus-check capability          # one target
cargo xtask verus-check --candidate         # prover/solver upgrade re-run only: certifies nothing
```

### Offline use (from the logs archive)

If you have the `verus-install/verus-x86-linux/` directory from the
verification logs archive, skip the download step and just add the
directory to `PATH`.

For full details including the rationale, two-toolchain design, and
conformance-only fallback, see
[`verification/verus/TOOLCHAIN.md`](https://github.com/nabbisen/fjell-os/blob/main/verification/verus/TOOLCHAIN.md).
