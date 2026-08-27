# Fjell OS

> **Every authority is explainable. Every update is verifiable. Every failure is
> recoverable.**

A capability-based microkernel for high-assurance edge and fleet nodes, written
in Rust (2024 edition) for `riscv64gc-unknown-none-elf`.

**This crate is the project's entry point on crates.io.** Fjell OS is an
operating system, not a library — you cannot depend on it to get an OS. What
this crate offers is a place to start reading, and a re-export of the one part
of the project that *is* a library: the stable ABI surface, as
[`fjell_os::abi`](https://docs.rs/fjell-os/latest/fjell_os/abi/).

## Where to go

| | |
|---|---|
| **Source** | <https://github.com/nabbisen/fjell-os> |
| **Changelog** | [`CHANGELOG.md`](https://github.com/nabbisen/fjell-os/blob/main/CHANGELOG.md) |
| **Roadmap** | [`ROADMAP.md`](https://github.com/nabbisen/fjell-os/blob/main/ROADMAP.md) |
| **RFCs** | [`rfcs/`](https://github.com/nabbisen/fjell-os/tree/main/rfcs) — every design decision, with its review record |
| **Known limitations** | [`docs/release/v1-limitations.md`](https://github.com/nabbisen/fjell-os/blob/main/docs/release/v1-limitations.md) |
| **Errata register** | [`docs/rfcs/ERRATA.md`](https://github.com/nabbisen/fjell-os/blob/main/docs/rfcs/ERRATA.md) — where every known divergence is recorded |
| **ABI crate** | [`fjell-abi`](https://crates.io/crates/fjell-abi) |

## Status — read this before depending on anything here

**Fjell OS is v0 software under active development. v1.0 is explicitly not in
view**, and functional advancement precedes any v1.0 consideration.

It runs on QEMU `virt`. It has never been booted on physical hardware — the
StarFive VisionFive 2 profile is provisional and unvalidated on silicon.

Neither this crate nor `fjell-abi` is yet intended as a stable dependency for
outside consumers. They are published so the project is findable and its ABI
surface inspectable.

## What the project is for

Fjell is for operators who need to answer three questions about every node in a
fleet:

- **What is running?** Every deployed binary is content-addressed and signed.
- **Who authorised it?** Every capability grant has a traceable, leased
  provenance.
- **How do I recover?** Every documented failure mode has a tested playbook.

Not for general-purpose servers, desktop environments, or POSIX workloads. See
the [v1.0 non-goals](https://github.com/nabbisen/fjell-os/blob/main/docs/release/v1-non-goals.md).

## How the project verifies itself

Releases are gated on twelve mechanical checks and a 21-tier test suite, and
every divergence between what a document claims and what shipped is recorded in
the errata register rather than fixed silently.

That register currently carries **13 accepted limitations and 0 open** — including
several found by auditing the verification tooling itself, which turned out to
have instruments that reported success without checking. See
[`docs/verification/instrument-audit-closeout.md`](https://github.com/nabbisen/fjell-os/blob/main/docs/verification/instrument-audit-closeout.md).

## Licence

Apache-2.0.
