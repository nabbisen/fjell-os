# ADR-v0.7-003 — Measurement and Release Summary Sync

**Status:** Accepted  
**Date:** 2026-05-19 (v0.7.0, RFC v0.7-003)

## Context

Other nodes need visibility into a peer's measurement chain and upgrade
counter state without receiving the full append-only log.

## Decision

Two summary types:
- `MeasurementSummary`: chain head digest, per-kind event tallies, policy
  digest. Domain: `"FJELL-MSUMMARY-V1"`.
- `ReleaseSummary`: per-channel current/min counters, active anchor epoch,
  last-confirm tick, advance source. Domain: `"FJELL-RSUMMARY-V1"`.

Both are signed via `attestd` and propagated through the snapshot-sync
channel. `summaryd` exports them periodically.

## Consequences

- Peers can verify measurement continuity without reading the full log.
- The signed summary is also usable as a fleet-health probe (v0.8).
- `AdvanceSource::SnapshotSync` makes it auditable when a counter was set
  by an incoming snapshot vs a local install.
