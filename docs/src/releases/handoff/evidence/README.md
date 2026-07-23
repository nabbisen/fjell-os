# Evidence

Gate logs are generated on demand rather than checked in, because the
reproducibility baseline is re-recorded per run by design and the QEMU
artifacts are ephemeral (both are gitignored).

To regenerate the full evidence set on the target machine:

```sh
cargo xtask build                  > evidence/build.log 2>&1
cargo xtask test-all               > evidence/test-all.log 2>&1
cargo xtask release-rehearsal      > evidence/release-rehearsal.log 2>&1
cargo xtask verus-check --release-required > evidence/verus.log 2>&1
```

Expected results are documented in `../testing-and-gates.md`.
