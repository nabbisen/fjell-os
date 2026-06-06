# RFC-v0.16-007: Runtime SDK Trial with fjell-config-sync

**Status:** Implemented (v0.16.0)
**Milestone:** v0.16 — Validation Closure
**Addresses:** architect review RB-08, H-05; errata E-006

## Problem

The v0.14 SDK trial proved `fjell-config-sync` compiles against the SDK
surface but never ran it. The ecosystem-readiness claim rested on a
library compilation, not a service lifecycle.

## Change

Added `crates/fjell-config-sync/tests/runtime_trial.rs` driving the
service through a full lifecycle via its real handler entry points:

1. Cold start (zero digest).
2. CONFIG_UPDATE → digest computed, counter advances, DigestReport reply.
3. Second distinct config → digest changes.
4. Idempotent re-apply → same digest, counter still advances.
5. CONFIG_QUERY → returns update count.
6. Semantic emit eligibility checked through the SDK `is_known_tag` path.
7. Unknown IPC tag → rejected.

Plus a convergence arm: two independent instances applying the same blob
produce the same digest (fleet-wide config convergence precondition).

## Markers

- `DRILL:SDK-CONFIG-SYNC-RUNTIME:PASS`
- `DRILL:SDK-CONFIG-SYNC-CONVERGENCE:PASS`

## Lessons added (extends docs/sdk/lessons-from-v0.14.md)

- **L5 [runtime]:** the handler dispatch exercised cleanly through
  `handle_ipc`; the SDK surface was sufficient to build a stateful
  service without reaching past the boundary.
- **L6 [convergence]:** digest determinism across instances held, which
  is what makes fleet config sync viable; this could not have been
  confirmed by the library-only v0.14 trial.

## Honesty

The trial drives the service's handler logic directly rather than over a
live kernel IPC switchboard in QEMU. It proves the service's runtime state
machine and SDK integration; it does not prove kernel-mediated IPC
delivery. The latter remains v1.x work, listed in v1.0 limitations.
