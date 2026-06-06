# ADR-v0.5-001 — PlatformProfile / BoardProfile Boundary

**Status:** Accepted  
**Date:** 2026-05-19 (v0.5.0, RFC v0.5-001)

## Context

`devmgr` previously contained hard-coded device tables for the QEMU `virt` machine.
Adding a second board or a second architecture required editing kernel code.

## Decision

Introduce two signed, content-addressable profile formats:
- **`PlatformProfile`** — architectural family, ISA extensions, memory map, PLIC layout.
- **`BoardProfile`** — device list (MMIO, IRQ, DMA), recovery descriptor, and a
  `platform_ref` binding it to a specific `PlatformProfile` digest.

Both are measured into the chain (`MeasurementKind::PlatformProfileLoaded`,
`MeasurementKind::BoardProfileLoaded`) and their digests enter the attestation record.
A new `KeyPurpose::BoardProfile` (0x07) covers the signing anchor.

## Consequences

- Adding a board requires only a new signed `BoardProfile` — no kernel recompile.
- The digest chain makes profile substitution detectable.
- `fjell-dtb-derive` derives `BoardProfile` from the kernel-handed-off DTB at boot,
  avoiding runtime FDT parsing in services.
- `devmgr` verifies `board.platform_ref == platform.profile_digest` before registration.
