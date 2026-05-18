# RFC 035: MmioRegion capability and MMIO ABI change

**RFC ID:** 035  
**Also known as:** RFC-v0.2-005  
**Status:** Proposed  
**Target version:** v0.2.0  
**Phase:** Phase 3 — MMIO Boundary Closure  
**Related epics:** C (MMIO/DMA), A (Unified Capability Enforcement)

## Problem

The v0.1.0 `sys_mmio_map` accepts a raw physical address and size:

```
sys_mmio_map(phys_addr, size)
```

Any holder of *any* MMIO-related authority can map *any* physical
range, including RAM, including ranges outside the kernel’s known
device map.  RFC 005 added a RAM-rejection guard, but the underlying
shape — *“give me a physical address and I will map it”* — is
unsafe.

RFC 030 (MMIO/DMA boundary audit) enumerates this as the central
v0.2 closure target.

## Proposed fix

Replace the ABI with a capability-bound region object:

```
sys_mmio_map(mmio_region_cap, offset, size)
```

### Kernel validation

Required checks at the syscall entry:

```
- CapKind::MmioRegion
- MMIO_MAP right
- lease state / epoch (RFC 033)
- offset + size within region.length
- region.phys_base + offset is *not* RAM
- mapping is user-accessible only for the owner task
- mapping is never executable
```

### Region object

```rust
pub struct MmioRegionObject {
    pub id:         MmioRegionId,
    pub owner:      TaskId,
    pub phys_base:  PhysAddr,
    pub length:     usize,
    pub lease_id:   LeaseId,
    pub state:      MmioRegionState,
}

pub enum MmioRegionState { Active, Revoked }
```

### Static region table (interim)

v0.2 uses a static `MmioRegionTable` populated at boot from a
platform descriptor (the existing `qemu_virt` constants plus DTB
where available).  Full DTB-driven device-manager registration is
deferred to v0.3 (Hardware Trust Abstraction) — designing it inside
v0.2 would couple boundary closure to a much larger work item.

### Removal of old ABI

The old `sys_mmio_map(phys_addr, size)` is removed from normal
builds.  A `cfg(debug_legacy_mmio)` gate may keep it for
transitional in-tree tests; CI must not enable this flag on release
builds.

## Rationale

- A capability-bound region object is the only way to make MMIO
  authority enforceable: the kernel can check ownership and lease
  state in O(1).
- Restricting the new ABI to *offset + size* within a pre-vetted
  region eliminates the entire class of "request the wrong physical
  address" bugs.
- Static `MmioRegionTable` is enough for the only driver Fjell OS
  ships at v0.2 (virtio-blk).  Generalising before there's a second
  device would be premature.

## Impact

- Crates: `fjell-kernel` (MMIO module, syscall entry, region table),
  `fjell-cap` (new CapKind), `fjell-driver-virtio-blk` (call site
  update), `fjell-devmgr` (region grant flow), `fjell-abi`
  (syscall number / signature).
- Backward compatibility: **breaks** the v0.1.x MMIO ABI.
  v0.2 is a breaking release; this is expected.
- The virtio-blk smoke path must be re-verified end-to-end.

## Test plan

### QEMU negative tests
- `NEG:MMIO:MAP_WITHOUT_CAP:PASS`
- `NEG:MMIO:MAP_RAM_REJECTED:PASS`
- `NEG:MMIO:OFFSET_OUT_OF_RANGE:PASS`
- `NEG:MMIO:REVOKED_REGION_REJECTED:PASS`

### Acceptance gates
- Normal builds contain no `sys_mmio_map(phys_addr, size)`.
- Only holder of the `MmioRegion` cap can map the region.
- `TEST:M6:PASS` continues to pass (virtio-blk smoke path).

## Implementation notes

- Out of scope: full DTB-driven region discovery (deferred to v0.3),
  IOMMU (deferred to v0.3), multi-region drivers (no current driver
  needs this).
- The "never executable" mapping is enforced by PTE bits at map
  time.  RFC 009 (W^X kernel pages) provides the precedent.
- The owner-task check pins the mapping to the task that holds the
  cap; transferring the mapping requires explicit cap copy/mint.
