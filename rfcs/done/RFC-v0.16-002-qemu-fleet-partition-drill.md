# RFC-v0.16-002: QEMU Fleet Partition Drill

**Status:** Implemented (v0.16.0)
**Milestone:** v0.16 — Validation Closure
**Addresses:** architect review RB-03; errata E-005 (partition arm)

## Problem

v0.13 shipped fleet-reliability *types* with unit tests of individual FSM
transitions, but no test exercised a complete partition lifecycle through
the runtime path. The handoff flagged this: "no actual fleet was ever
partitioned and reconciled."

## Change

Added `crates/fjell-fleet-sync/tests/partition_drill.rs`, an integration
drill that drives the full cycle:

1. Healthy fleet baseline.
2. Heartbeat loss → `Suspect` → `Partitioned` (FSM-guarded; illegal
   transitions panic the drill).
3. Divergent record acceptance on both sides during the partition.
4. Link restore → `Reconciling`.
5. Coordinator builds a `ReconcileManifest` (coordinator records Accepted,
   partition-side authority conflict Rejected).
6. Member applies the manifest → `Healthy`.
7. Post-rejoin summary consistency check (no seq regression).

A negative arm proves a rollback attack (regressed `sync_seq` at rejoin)
is detected by the consistency checker.

## Markers

- `DRILL:FLEET-PARTITION-RECONCILE:PASS`
- `DRILL:FLEET-PARTITION-ROLLBACK-REJECTED:PASS`

## Scope honesty

This drill is **host-simulated**, not a multi-VM QEMU topology. It
exercises the real `fjell-fleet-sync` runtime APIs end-to-end through a
full lifecycle — a substantial step beyond per-transition unit tests —
but it does not validate wire-level heartbeat transport or real network
partition behaviour. Multi-VM partition testing remains v1.1 work and is
listed in the v1.0 limitations (RFC-v0.16-005).

## Test plan

`cargo test -p fjell-fleet-sync --test partition_drill` → 2 pass, both
markers emit under `--nocapture`.
