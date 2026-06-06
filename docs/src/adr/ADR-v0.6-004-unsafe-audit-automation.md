# ADR-v0.6-004 — Unsafe Boundary Inventory and Audit Automation

**Status:** Accepted  
**Date:** 2026-05-19 (v0.6.0, RFC v0.6-004)

## Context

Fjell OS contains unsafe Rust in kernel and driver code. Without systematic
tracking, unsafe sites accumulate without review and invariants go undocumented.

## Decision

`tools/fjell-unsafe-audit` scans the workspace for unsafe blocks, functions, impls,
and traits. For each site it checks for a `// SAFETY:` comment within 4 preceding
lines. CI runs `fjell-unsafe-audit --check` on every PR; the build fails if any
unsafe site lacks a comment.

At v0.6.0: 261 unsafe sites, 261 covered (100%). The `UNSAFE_CHARTER.md` at the
repo root documents the policy, permitted patterns, and prohibited patterns.

## Consequences

- New unsafe sites require a SAFETY comment before the PR is merged.
- The inventory is automatically regenerated and checked rather than manually
  maintained — eliminating drift.
- Reviewers know exactly where to focus security attention.
