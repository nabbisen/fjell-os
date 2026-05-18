# RFC 013: Create ADRs 0006–0010 for M6/M7 design decisions

**RFC ID:** 013  
**Status:** Accepted (documentation work, start M7.1)  
**Affects:** `docs/src/adr/`

## Problem (M-05)

No ADRs exist for M6 (device driver model, DMA, virtio) or M7 (trust model,
snapshot model, inline init workaround).

## Required ADRs

| ADR | Title |
|---|---|
| 0006 | User-space driver model and MMIO/DMA capability boundary |
| 0007 | Persistent append-only store and recovery model |
| 0008 | Verified immutable rootfs and signed artifact model |
| 0009 | A/B boot-control and health-based confirmation model |
| 0010 | Inline init smoke workaround and service separation plan |

## Format

Follow existing ADRs (0001–0005): Context, Decision, Consequences, Status.

## Defer condition

Complete before M8 sprint begins.
