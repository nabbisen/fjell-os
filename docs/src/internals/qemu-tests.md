# QEMU Tests

Fjell OS uses UART-based smoke tests to gate each milestone.

## Running interactively

```sh
cargo xtask qemu
# Press Ctrl-A X to exit QEMU
```

## Running a smoke test

```sh
cargo xtask qemu-test        # M1 — checks for "Fjell OS kernel started"
cargo xtask qemu-test m2     # M2 — checks for TEST:M2:PASS
cargo xtask qemu-test m3     # M3 — checks for TEST:M3:PASS
```

The tool builds `fjell-kernel` in release mode, runs it under
`qemu-system-riscv64 -machine virt -bios none -nographic` with a 10-second
timeout, and scans UART output for the expected marker string.

## M1 expected output

```
=============================
  Fjell OS kernel started.
=============================

arch  : riscv64
mach  : qemu-virt
stage : M1 bootable kernel
```

## M2 expected output (target)

```
Fjell OS kernel started.
mode: S
platform: qemu-virt
mm: boot allocator ready
mm: frame allocator ready
vm: sv39 enabled
trap: stvec installed
task: idle created
task: user0 created
task: user1 created
sched: started
user0: yield
user1: yield
user0: exit(0)
user1: fault(load page fault)
sched: idle
TEST:M2:PASS
```

## CI integration

Smoke tests run automatically in `.github/workflows/ci.yml` on every push
to `main` and all `dev/**` branches.
