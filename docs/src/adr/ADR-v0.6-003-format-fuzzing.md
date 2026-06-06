# ADR-v0.6-003 — Format Fuzzing and Frozen Schema Registry

**Status:** Accepted  
**Date:** 2026-05-19 (v0.6.0, RFC v0.6-003)

## Context

Binary format parsers are historically the richest source of exploitable bugs.
Fjell OS has 8 critical formats that cross service boundaries and survive across
reboots.

## Decision

A `fuzz/` directory using `cargo +nightly fuzz` contains 8 targets. Each target
verifies that the parser never panics on arbitrary input and that parse → serialize
→ parse produces an identical result.

Frozen schema files lock field layouts. Any layout change must be accompanied by
a BREAKING-SCHEMA commit, a schema version bump, and an ADR — enforced by CI.

Fuzzing runs nightly with the seeded corpora as starting points.

## Consequences

- Format regressions that cause parser panics are caught before merge.
- Schema drift (accidental field reorder, size change) is caught per-PR.
- The frozen schema files serve as authoritative wire-format documentation.
