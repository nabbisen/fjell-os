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

> **Correction, 2026-09-15 (E-043).** The harness described here has never
> run. The CI job is weekly, not nightly, and runs only on its schedule —
> never on push or pull request — so it could not catch a regression "before
> merge" even when working. It has failed every week since it was added:
> `fuzz/` is misconfigured as a workspace member, its dependency paths broke in
> the July 2026 crate reorganisation, and six of the eight targets call parser
> functions that no longer exist. The decision stands; the consequences below
> have not held.

## Consequences

- Format regressions that cause parser panics are caught before merge.
- Schema drift (accidental field reorder, size change) is caught per-PR.
- The frozen schema files serve as authoritative wire-format documentation.

> **Correction, 2026-09-15 (E-045).** Neither consequence about schemas has
> held. `ci-schema-gate` checks only that the frozen files exist and are not
> empty; the `fjell-tools schema dump` generator and the comparison test that
> RFC-v0.6-003 specified were never built; and in both formats checked, the
> frozen description no longer matches the code, with no schema version bumped.
