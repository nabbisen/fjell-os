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

## CI integration

All jobs run automatically in `.github/workflows/ci.yml` on every push to
`main` and on pull requests.  Jobs:

- `ci-format` — `cargo fmt --check`
- `ci-check` — host-buildable crates (format, cap, ipc, syscall, tools, ...)
- `ci-cross-check` — RISC-V cross-build check (kernel + service binaries)
- `ci-test-host` — host unit tests (14 policy + 16 cap + 10 ipc)
- `ci-docs` — `mdbook build`
- `ci-qemu-smoke` — all 8 milestone smoke tests in parallel
- `ci-qemu-negative` — all 9 negative-test categories in parallel
