# ADR-v0.6-001 — Capability/IPC/Lease Property-Test Harness

**Status:** Accepted  
**Date:** 2026-05-19 (v0.6.0, RFC v0.6-001)

## Context

v0.2 defined the capability/IPC/lease invariants but did not prove them automatically.
Manual code review is insufficient for the combinatorial space of revocation sequences.

## Decision

`fjell-proptest` implements 10 invariant properties against a pure in-process model
(`ModelState`), driven by `proptest` with 1000 random operation sequences per property.
The model is separate from the kernel runtime; it validates the *design*, not the code.

Discovered failures shrink to minimal sequences via proptest's built-in shrinker.
Regression seeds are committed so the harness never regresses silently.

The master seed `0x46656C6C` ("Fell") ensures reproducibility across environments.

## Consequences

- Any future change to cap/IPC/lease semantics must first update `ModelState` and pass
  all 10 properties before touching kernel code.
- The harness runs in under 10 minutes, making it CI-friendly.
- Property failures are human-readable sequences, not opaque binary diffs.
