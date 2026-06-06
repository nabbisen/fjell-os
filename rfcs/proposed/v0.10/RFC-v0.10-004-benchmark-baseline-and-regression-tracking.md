# RFC-v0.10-004 — Benchmark Baseline and Regression Tracking

**Status:** Proposed
**Target version:** v0.10.0
**Parent:** RFC 061 (credibility for edge claims).

## 1. Problem

Fjell makes implicit performance claims by virtue of targeting edge
nodes: small kernel, fast capability checks, low-overhead IPC, modest
memory. None of these claims are *measured*. v0.10 fixes this by
landing a baseline and a regression detector.

The goal is not to chase peak throughput. The goal is to catch
silent regressions and to publish honest numbers operators can plan
against.

## 2. What to measure

### 2.1 Kernel micro-benchmarks (host-portable)

- `require_cap` check latency (cycles + ns).
- `sys_ipc_call` round-trip (cycles).
- Capability copy (`sys_cap_copy`) + drop (`sys_cap_drop`) cycle.
- Lease creation + revocation cycle.
- Semantic intent emit (catalog v1 encode).
- Audit record append.

### 2.2 Boot and footprint

- Kernel image size (text/rodata/data/bss, post-objcopy).
- Kernel + 26 services compressed bundle size.
- Time from `_start` to first `init: ready` (cycles, in QEMU).
- Time from boot to `TEST:M8:PASS` (wall-clock).

### 2.3 Format encode/decode throughput

- Audit log encode/decode (records/sec).
- Bundle digest computation (MB/sec).
- Semantic catalog v1 round-trip throughput.

## 3. Harness

Two sub-harnesses:

### 3.1 Host criterion benches

For pure code paths (S5 audit, bundle digest, semantic encode, lease
math, cap-broker policy eval): standard `criterion` benches under
`benches/`. These run on the host x86-64 in seconds.

### 3.2 QEMU instrumentation

For kernel paths: a new `tests/qemu/profiles/bench.toml` that boots a
kernel built with a `kbench` feature exposing cycle counters on each
syscall. A `fjell-bench` service runs the workloads and emits results
as semantic intents that the host xtask collects.

The QEMU bench is intentionally cycle-counter-based; wall-clock would
be too noisy under emulation.

## 4. Baseline format

`docs/perf/baseline.json` checked into the repo, keyed by commit:

```json
{
  "schema": 1,
  "commit": "<sha>",
  "host": "<rustc + target>",
  "metrics": {
    "require_cap_ns":      { "median": 120, "p99": 180, "tol_pct": 15 },
    "ipc_call_cycles":     { "median": 1450, "p99": 1700, "tol_pct": 10 },
    "kernel_text_bytes":   { "value": 2_500_000, "tol_pct": 2 },
    "..."
  }
}
```

Each metric has a tolerance band. `tol_pct: 10` means a value more
than 10% worse than the recorded median fails the regression check.

## 5. CI gate

A new tier in `cargo xtask test-all`:

- `cargo xtask bench --baseline` produces a current measurement file.
- The xtask compares against `docs/perf/baseline.json`.
- Regressions outside tolerance fail the build.
- Improvements outside tolerance log a notice; the baseline is **not**
  auto-updated. A separate PR explicitly bumps the baseline.

## 6. Honest reporting

The baseline file is published. The README and `docs/perf/baseline.md`
publish a table of current numbers with caveats:

- QEMU cycle counts are emulation artefacts, useful for relative
  comparison only.
- Host criterion numbers assume an x86-64 dev box, not the target
  RISC-V silicon.
- v0.12 (first real-board profile) re-publishes baselines on hardware.

## 7. Acceptance criteria

1. `benches/` directory exists with at least 6 host benches.
2. `tests/qemu/profiles/bench.toml` + `fjell-bench` service exist and
   produce results.
3. `docs/perf/baseline.json` is committed and version-1 schema valid.
4. `cargo xtask bench` runs, compares, and exits non-zero on regression.
5. The README links to `docs/perf/baseline.md` with current numbers
   and the caveats from §6.

## 8. Out of scope

- Hardware benchmarking (v0.12).
- Energy / power measurements (research track).
- Continuous benchmark dashboards (v0.13 or later).
- Comparison against Linux or other kernels.
