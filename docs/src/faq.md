# Frequently Asked Questions

## Is Fjell a POSIX OS?

No. Fjell does not implement `read(2)`, `write(2)`, `fork(2)`, or file descriptors. Authority is granted via capability handles, not ambient descriptors. See [Non-Goals](./intro/non-goals.md).

## Can I run Linux software on Fjell?

Not directly. Linux software expects POSIX semantics. Fjell services are authored against `fjell-sdk`. See [Writing a Service](./sdk/writing-a-service.md).

## What architectures are supported?

v0.9: RISC-V RV64GC (QEMU `virt`). v0.12 adds the first real RISC-V board. ARM64 is deferred post-v1.0.

## Where is the kernel source?

`crates/fjell-kernel/` — all code in Rust with `#![forbid(unsafe_code)]` except audited boundaries under `UNSAFE_CHARTER.md`.

*TODO: expand as questions arise.*
