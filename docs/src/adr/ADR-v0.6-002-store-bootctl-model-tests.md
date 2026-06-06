# ADR-v0.6-002 — Store Recovery and Boot-Control State-Machine Model Tests

**Status:** Accepted  
**Date:** 2026-05-19 (v0.6.0, RFC v0.6-002)

## Context

The append-only store (storaged) and boot-control (bootctl) have subtle state
machines that are hard to test through integration alone. In particular, crash
recovery and slot-fallback sequences interact non-trivially.

## Decision

Two model crates (`fjell-store-model`, `fjell-bootctl-model`) implement the state
machines in pure Rust with `proptest`-driven property testing. Six store properties
and six bootctl properties cover the critical invariants.

Proptest found three model bugs during development (b2, b4, b6), which were fixed
before the harness was accepted. The bug-fixing process validates the harness's
effectiveness.

## Consequences

- Store recovery logic changes must be validated against `fjell-store-model` first.
- The `BOOT_COUNT_MAX` overflow path is exercised on every CI run.
- Model-level tests are order-independent of QEMU availability.
