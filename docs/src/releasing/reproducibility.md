# Reproducible Builds

All release artefacts are bit-for-bit reproducible given the same `rust-toolchain.toml`,
`RUSTFLAGS=--remap-path-prefix=$PWD=.`, **and the committed `[profile.release]`**.

**The profile is one of the inputs, not a detail.** In particular `codegen-units = 1` is
load-bearing: at Cargo's default of 16 units, fat LTO leaves the order in which LLVM numbers its
internal symbols to a parallel race, and about **7% of builds of the kernel produced a second,
different binary** — the same program with different internal numbering. Two clean builds of one
tree then disagree and this page's claim has no grounds. Pinned, one unit gives one output across
150 interleaved builds and independently of `CARGO_BUILD_JOBS`. The knob carries the same warning
at its definition in the root `Cargo.toml`; **removing it silently revokes this claim.** *(Errata
E-107, closed 2026-11-04.)*

Gate: `cargo xtask repro-check` — two builds of the whole tree, each preceded by a scoped
`cargo clean`, comparing **34 artefacts**: the kernel and the 33 service binaries, all of which
`cargo xtask build` regenerates. On a mismatch both builds' artefacts are retained under
`.git-exclude/` so the difference can be read from the binaries and not only from their digests.

*References RFC-v0.10-003.*
