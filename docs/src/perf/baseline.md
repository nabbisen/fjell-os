# Fjell OS — Performance Baseline

*Governed by RFC-v0.10-004. Numbers are criterion medians on the host
build machine (x86-64). Not RISC-V silicon numbers; see §Caveats.*

## v0.9.4 baseline

| Benchmark | Median | Tolerance |
|-----------|--------|-----------|
| `audit/from_bytes` | 7.7 ns | ±20% |
| `bundle/build_bundle/4kib` | 20.3 µs | ±15% |
| `bundle/build_bundle/1mib` | 5.0 ms | ±15% |
| `cap/require_cap/ok` | 9.0 ns | ±20% |
| `cap/require_cap/wrong_handle` | 6.8 ns | ±20% |
| `manifest/parse` | 1.2 µs | ±20% |
| `manifest/lint` | 184 ns | ±20% |
| `manifest/parse_and_lint` | 1.5 µs | ±20% |
| `semantic/encode` | 34 ns | ±20% |
| `semantic/decode` | 235 ns | ±20% |

## Caveats

- All numbers are x86-64 host benchmarks, not RISC-V targets.
- QEMU cycle-count instrumentation (`fjell-bench` service, RFC-v0.10-004 §3.2)
  is scheduled for v0.10 and will add kernel-path metrics.
- Hardware baseline on the chosen v0.12 board replaces these as the
  authoritative performance reference at v0.12.
- Tolerance bands are deliberately wide for the v0.9.4 baseline.
  They will be tightened once the build environment is stabilised.

## Regression policy

`cargo xtask bench` compares against `docs/perf/baseline.json`.
A metric exceeding its `tol_pct` band fails the build.
Improvements exceeding the band log a notice but do not fail;
updating the baseline requires a deliberate PR.

See `benches/` for the criterion harness and
[RFC-v0.10-004](../../rfcs/proposed/v0.10/RFC-v0.10-004-benchmark-baseline-and-regression-tracking.md)
for the full specification.
