# QEMU Tests

Fjell OS uses two test harnesses under QEMU: milestone smoke tests
and category-based negative tests.

## Running interactively

```sh
cargo xtask qemu
# Press Ctrl-A X to exit QEMU
```

## Milestone smoke tests

Each milestone gate has a unique `TEST:Mn:PASS` token emitted to UART.

```sh
cargo xtask qemu-test        # current milestone (M8)
cargo xtask qemu-test m8     # explicit — checks for TEST:M8:PASS
cargo xtask qemu-test m4     # earlier milestone
```

The tool builds `fjell-kernel` in release mode, runs it under
`qemu-system-riscv64 -machine virt -bios none -nographic` with a configurable
timeout, and scans UART output for the expected token.

## Negative tests

Negative tests verify that the kernel rejects bad operations with the correct
error codes.  Each test category has a TOML profile in `tests/qemu/profiles/`
listing the expected `NEG:*:PASS` markers.

```sh
cargo xtask qemu-negative capability   # 8 markers — cap enforcement (RFC 031/049)
cargo xtask qemu-negative mmio         # 3 markers — MMIO boundary (RFC 035)
cargo xtask qemu-negative dma          # 3 markers — DMA boundary (RFC 036)
cargo xtask qemu-negative user-copy    # 2 markers — UserPtr rejection (RFC 039)
cargo xtask qemu-negative policy       # 4 markers — cap-broker policy (RFC 040/055)
cargo xtask qemu-negative audit        # 1 marker  — audit ring evidence gap (RFC 041)
cargo xtask qemu-negative ipc          # 3 markers — IPC lease revocation (RFC 034)
cargo xtask qemu-negative svc          # 4 markers — service lifecycle (RFC 038/058)
cargo xtask qemu-negative harness      # 1 marker  — CSpace layout self-check (RFC 050)
```

A test run for a single category fails if any expected marker is absent or if
the build produces any warning or error.

### A machine that is meant to stop

Most profiles end because the harness's timeout killed QEMU after the markers
had appeared. A profile that tests a **reset** cannot work that way: a reset in
a test looks, to a harness matching markers in a serial log, like a machine that
stopped. Measured against a scratch probe that prints `BOOT` and then stores to
QEMU `virt`'s `sifive_test` device:

| probe | flags | exit | `BOOT` lines | QEMU's `SHUTDOWN` event |
|---|---|---:|---:|---|
| hang | `-no-reboot` | 124 | 1 | `guest: false`, `host-signal` |
| reset (`0x7777`) | *(none)* | 124 | **32,086** | *(killed first)* |
| reset (`0x7777`) | `-no-reboot` | 0 | 1 | `guest: true`, `guest-reset` |
| power-off (`0x5555`) | `-no-reboot` | 0 | 1 | `guest: true`, `guest-shutdown` |

Without `-no-reboot`, a reset is a **boot loop** that ends as a timeout kill —
exit 124, exactly like a hang. With it, a reset and a power-off both exit 0, so
a kernel that wrote the wrong value would read as success. QEMU itself says
which, through the `SHUTDOWN` event on its QMP socket.

A profile opts in with one key:

```toml
expect_shutdown = "guest-reset"     # or "guest-shutdown"
```

The runner then adds `-no-reboot` and a QMP socket, and the run **fails unless
QEMU reports `SHUTDOWN` with `guest: true` and that reason, and exited by itself
rather than being killed by the timeout**. The machine's own account is the
evidence; no guest marker asserts a reset. The outcome is recorded in
`runs/<run-id>/qemu-shutdown.txt`. A misspelt value is refused when the profile
loads, and `host-signal` — what a hang looks like — is never an outcome to
expect. Every other profile is unchanged.

The same probe run through the runner *without* the key passes for both the
reset and the hang, because `BOOT` appeared. That is the gap this closes.

## CI integration

All jobs run automatically in `.github/workflows/ci.yml` on every push to
`main` and on pull requests.  Jobs:

- `ci-format` — `cargo fmt --check`
- `ci-check` — host-buildable crates (format, cap, ipc, syscall, tools, ...)
- `ci-cross-check` — RISC-V cross-build check (kernel + service binaries)
- `ci-test-lib` — every crate's lib tests, workspace-derived (`cargo xtask host-lib-tests`)
- `ci-docs` — `mdbook build`
- `ci-qemu-smoke` — all 8 milestone smoke tests in parallel
- `ci-qemu-negative` — all 9 negative-test categories in parallel
