# Verus Setup

The complete Verus installation recipe, pinned version, and background
are documented in
[`verification/verus/TOOLCHAIN.md`](../../../verification/verus/TOOLCHAIN.md)
at the repository root.

## Quick reference

Verus is **optional** — the kernel and all crates build without it.
It is only needed for Gate 10 (`release-rehearsal`) and explicit
`cargo xtask verus-check` runs.

### Pinned version

```
verus:     release/0.2026.05.24.ecee80a
toolchain: 1.95.0-x86_64-unknown-linux-gnu  (separate from Fjell's 1.91)
z3:        4.12.5 (bundled in the release asset)
```

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
```

### Offline use (from the logs archive)

If you have the `verus-install/verus-x86-linux/` directory from the
verification logs archive, skip the download step and just add the
directory to `PATH`.

For full details including the rationale, two-toolchain design, and
conformance-only fallback, see
[`verification/verus/TOOLCHAIN.md`](../../../verification/verus/TOOLCHAIN.md).
