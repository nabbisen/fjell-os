# fjell-repro-check

Reproducible-build gate (RFC-v0.16-005, H-04: SHA-256 digests).

## Modes

- **Full** (`cargo xtask repro-check`): builds the riscv64 services + kernel
  twice and compares every artefact digest. This is the real reproducibility
  guarantee.
- **`--skip-build`**: compares the committed `prebuilt/*.bin` against the
  committed baseline `tests/repro/baseline-digests.txt` (fast; used as
  test-all tier 5). `target/` artefacts are deliberately excluded — they are
  volatile across `cargo clean` and fresh checkouts.

## Baseline maintenance (IMPORTANT)

The baseline tracks the committed prebuilt service binaries. **Whenever the
prebuilt binaries are rebuilt** (any `cargo xtask build`, `build-services`,
or `qemu-test` run after a source change that affects services), the
baseline must be re-recorded and committed together with the new binaries:

```bash
rm tests/repro/baseline-digests.txt
cargo xtask repro-check --skip-build    # re-records, prints "baseline written"
cargo xtask repro-check --skip-build    # second run must print PASS
git add crates/fjell-kernel/prebuilt tests/repro/baseline-digests.txt
```

The baseline header records the digest algorithm (`# algo: sha256`); a
legacy pre-H-04 FNV baseline is rejected loudly with re-record instructions
rather than producing meaningless cross-algorithm diffs.
