# ADR-v0.5-002 — No Runtime DTB Parsing in User-Space Services

**Status:** Accepted  
**Date:** 2026-05-19 (v0.5.0, RFC v0.5-002)

## Context

DTB parsing is error-prone, requires a heap or large stack allocation, and is a
historically rich source of exploitable bugs in embedded OS code.

## Decision

DTB parsing is done exactly once: at `devmgr` boot, using `fjell-dtb-derive`.
The result is a `BoardProfile` (a fixed-size, heap-free struct).  All other
services consume the `BoardProfile`; they never see raw DTB bytes.

The `fjell-dtb-derive` parser is minimal — it handles only the node/prop tokens
needed to extract device class, MMIO range, IRQ line, and `compatible` strings.
Unknown `compatible` values produce `DeriveError::UnknownNode`; the service
fails hard rather than registering an unknown device.

## Consequences

- DTB complexity is isolated to a single, well-tested library.
- Services have a typed, bounded device list with no pointer arithmetic.
- Unknown hardware is rejected at boot rather than silently ignored.

> **Correction, 2026-09-15 (E-048).** None of this was built. `devmgr` parses
> no device tree — it constructs `BoardProfile::qemu_virt_default` in code — and
> no crate uses `fjell-dtb-derive`. `DeriveError::UnknownNode` does not exist:
> an unknown `compatible` is **skipped**, not rejected. The parser has never
> derived a profile from a real tree; on QEMU's own `virt` tree it returns
> `MissingPlic`, because QEMU nests devices under `/soc` one level deeper than
> it looks. Its tests pass on synthetic trees built to its own assumption. The
> decision — no runtime DTB parsing in user-space services — is untouched, and
> holds today for the simpler reason that nothing parses a DTB at all. The
> crate's disposition is an owner decision.
