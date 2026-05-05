# What is Fjell OS?

Fjell OS is a research operating system that demonstrates a specific thesis:

> A memory-safe minimal kernel, capability-based IPC, user-space services,
> append-only audit, declarative configuration, and a semantic intent stream
> can form a small, correct, and extensible OS.

## Key properties

**Memory-safe by construction** — the kernel is written in Rust 2024 edition
with `#![no_std]`.  Memory bugs are eliminated at compile time, not patched
at runtime.

**Minimal trusted computing base** — the kernel implements only five things:
address-space isolation, task management, IPC, capability enforcement, and
interrupt handling.  Everything else runs in user space.

**No ambient authority** — there is no `root`.  Every access requires an
explicit capability token.  A compromised driver can only affect what it was
explicitly granted permission to touch.

**Readable state** — all significant kernel transitions are recorded in an
append-only audit ring exportable as JSON Lines.

**Semantic interface (ABDD)** — applications emit structured intent rather
than pixel data.  A Presentation Proxy translates that intent for any
modality: text, speech, braille, or machine API.

## What it is not

Fjell OS is not a general-purpose desktop OS, not a Linux replacement, and
not an AI-native kernel.  See [ADR-0005](../adr/0005-v010-scope.md) for the
explicit scope boundaries.

## Current status

v0.1.0 is in active development.  See the [ROADMAP](../../ROADMAP.md) for
the milestone schedule.
