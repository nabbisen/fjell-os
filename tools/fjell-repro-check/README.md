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

**Fail-closed (RFC-v0.21.3-001 §M4):** a missing baseline is a **FAIL**, not
a free pass. Recording a baseline is an explicit, opt-in action —
`--record-baseline` — and is never a side effect of an ordinary check run.
Before this fix, an absent baseline silently recorded itself and reported
PASS, so the check-mode tier detected nothing until a baseline first existed.

The baseline tracks the committed prebuilt service binaries. **Whenever the
prebuilt binaries are rebuilt** (any `cargo xtask build`, `build-services`,
or `qemu-test` run after a source change that affects services — or, per
Finding C below, sometimes with no source change at all), the baseline must
be re-recorded and committed together with the new binaries:

```bash
rm tests/repro/baseline-digests.txt
cargo xtask repro-check --record-baseline    # explicit opt-in; prints "baseline recorded"
cargo xtask repro-check                      # second run must print PASS
git add crates/fjell-kernel/prebuilt tests/repro/baseline-digests.txt
```

Running `cargo xtask repro-check` (or `--skip-build` directly against the
binary) with no baseline present, and without `--record-baseline`, now fails
with a message naming the recording command — it does not record anything.
Passing `--record-baseline` when a baseline already exists is also refused;
delete the file first if you intend to re-record over it.

The baseline header records the digest algorithm (`# algo: sha256`); a
legacy pre-H-04 FNV baseline is rejected loudly with re-record instructions
rather than producing meaningless cross-algorithm diffs.

## Known limitation — Finding C (build-output non-determinism)

At least 9 of the 28 prebuilt service binaries have been observed to change
byte-for-byte between rebuilds of *identical* source, at identical file
sizes (see `review-record-slice-2b-2c.md` under
`rfcs/handoffs/RFC-v0.21.3-001-build-restoration-and-as-built-reconciliation/`
for the full characterization). This does not affect the correctness of this
tool or the baseline it records — the baseline tracks whatever is currently
committed, not an assumption that rebuilds are deterministic — but it does
mean **rebuilding the prebuilt binaries can require a baseline re-record even
when no source changed**. The durable fix (build/link determinism) is
out of scope for this tool and is tracked as a v0.22 candidate, its own RFC.
