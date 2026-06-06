# Reproducible Builds

All release artefacts are bit-for-bit reproducible given the same `rust-toolchain.toml` and `RUSTFLAGS=--remap-path-prefix=$PWD=.`.

Gate: `cargo xtask repro-check`

*References RFC-v0.10-003.*
