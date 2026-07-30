# Verus Toolchain

Verus is **not** a Fjell build dependency. The kernel and all crates build
and test with the stable toolchain in `rust-toolchain.toml`. Verus is an
*optional* proof checker for the pilot targets in `verus-targets.toml`.

## Pinned version

```
verus:     release/0.2026.07.27.31579f0   (AUR package `verus-bin`)
toolchain: 1.97.1-x86_64-unknown-linux-gnu (rustup; required by the Verus binary)
z3:        4.12.5                           (bundled with the Verus package)
```

Updated 2026-07-30 (v0.21.3): upgraded from the hand-unpacked release asset
`release/0.2026.05.24.ecee80a` / rustup 1.95.0 to the package-managed AUR
`verus-bin` / rustup 1.97.1. All 20 obligations were re-verified unchanged
under the new prover, on a different rustc and a different host OS — see
`docs/verification/verus/review-records/v0.21.3-prover-upgrade.md` for the
recorded re-certification. The previous pin's certification is not
retracted; both are recorded in `TOOLCHAIN.lock` (`[history]`).

The exact pin lives in `verification/verus/TOOLCHAIN.lock` for
reproducibility. Note the Verus toolchain (rustup) is independent of the
Fjell build toolchain (`rust-toolchain.toml`, channel 1.91, apt) — Verus is
never a Fjell build dependency.

## Install (developer / CI)

**Recommended: AUR `verus-bin`** (Arch / CachyOS) — package-managed,
self-contained, and the currently-pinned method:

```bash
# provides /usr/bin/verus; also installs the rustc toolchain it needs
paru -S verus-bin   # or: yay -S verus-bin
verus --version     # → 0.2026.07.27.31579f0
```

**Alternative: hand-unpacked GitHub release** (any Linux x86_64; not
package-managed, so updates and integrity are the installer's own
responsibility). Get the release matching the pin above
(`release/0.2026.07.27.31579f0`) from
https://github.com/verus-lang/verus/releases — the exact worked recipe
below is for the *previous* pin and is kept as a pattern to follow, not as
current download commands; verify the asset name against whichever version
you actually need before running it.

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

If `verus` is not on PATH, `verus-check` runs each target's Rust
conformance test instead and reports `VERUS:TARGET:<name>:CONFORMANCE-ONLY`.

**This does not mean "no blocker."** `capability` and `lease` were promoted
to `release_required = true` at v0.18.0 (RFC-v0.18-001): for those two
targets, `cargo xtask verus-check --release-required` — and therefore Gate
10 of `release-rehearsal` — **fails** on `CONFORMANCE-ONLY`, exactly as it
would on a proof error. A gate that cannot run is not a passing gate
(RFC-v0.21.3-002). Only `boot-control` remains pilot-only
(`release_required = false`), where conformance-only is a real, non-blocking
outcome. Ordinary `cargo build` / `cargo test` never need Verus — only
cutting a release does.

## Worked hand-unpack recipe for the *previous* pin (pattern reference only)

This installed `release/0.2026.05.24.ecee80a` / rustup 1.95.0, retired
2026-07-30 in favour of the AUR package above. **Not current** —
`verus-check --release-required` checks the detected version against the
pin in `TOOLCHAIN.lock` and refuses to certify on a mismatch, passing
proofs notwithstanding. Kept only to show the shape of a manual install for
whoever adapts it to the current release asset:

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

verus --version   # → 0.2026.05.24.ecee80a / toolchain 1.95.0 (previous pin)
```

History: at v0.17.0 the build sandbox could not reach the GitHub release-asset
hosts, so the proofs were temporarily validated by conformance + property
tests only. The hosts were reachable from v0.17.1, and the prover moved to
the package-managed AUR `verus-bin` at v0.21.3 (see `TOOLCHAIN.lock`
`[history]` and the v0.21.3 proof-review re-run record).
