# RFC 030: MMIO / DMA boundary audit

**RFC ID:** 030  
**Also known as:** RFC-v0.1.x-007  
**Status:** Implemented (v0.1.3)
**Target version:** v0.1.3  
**Affects:** `docs/src/audit/`, no code

## Problem

MMIO and DMA are among the highest-risk parts of any operating system:
the kernel hands a user-space driver direct hardware access, and any
mistake bypasses every other security boundary.

Even if v0.1.0 is a prototype that only drives virtio-blk, the project
must document exactly how its MMIO mapping and DMA allocation behave,
which caller-controlled inputs are accepted, and what is currently
unsafe.

## Proposed fix

Produce `docs/src/audit/mmio-dma-boundary-audit-v0.1.md` with the
following sections:

1. **MMIO current model** — how `sys_mmio_map` works today, which
   addresses it accepts, which checks it performs, which it does not.
2. **DMA current model** — how `sys_dma_alloc` works today, what
   ownership/revocation/zeroize behaviour exists, what is missing.
3. **Threats** — what an in-task attacker (a malicious or buggy
   driver) could currently do.
4. **Current mitigations** — what already prevents the worst
   outcomes.
5. **Known gaps** — what is *not* mitigated and is currently relying
   on driver good behaviour.
6. **Required v0.2 fixes** — see list below.
7. **Negative tests** — names from RFC 026 (MMIO / DMA categories).
8. **Deferred work** — items that wait beyond v0.2.

### Required v0.2 fixes (entered as v0.2 backlog by RFC 034)

```
- MmioRegion capability                       (RFC 016 reference)
- DmaRegion capability                        (RFC 017 reference)
- 1-page DmaRegion limit until scatter-gather design
- owner-task tracking on every region
- lease binding on every region
- revoke state and use-time check
- zeroize on revoke / task exit
- quarantine timeout for revoked DMA pages
- RAM range rejection in mmio_map           (RFC 005 reference)
- offset/length bounds check                (RFC 005 reference)
```

## Rationale

A single audit document forces every MMIO / DMA decision to be made
once, in writing, before any code changes.  The Required v0.2 fixes
list is the contract v0.2 must satisfy.

The 1-page DMA limit is intentionally conservative.  Scatter-gather
support is a deliberate v0.3+ topic; trying to design it in v0.2 would
delay security closure for a feature that the current driver
(virtio-blk) does not require.

## Impact

- Documentation only.  No code changes.
- References existing RFCs 005, 007, 016, 017.
- Establishes the v0.2 work contract for the MMIO/DMA epic.

## Test plan

- Document exists.
- Each of the eight required sections has at least a paragraph.
- Every named MMIO / DMA negative test in RFC 026 appears in §7.
- v0.2 roadmap links to this document.

## Implementation notes

- No full device-manager redesign in v0.1.x.
- No IOMMU implementation.  IOMMU is a v0.3+ topic and depends on the
  HardwareTrustProvider abstraction.
- No multi-page DMA scatter-gather yet — see above.
