# RFC 016: MmioRegion capability + sys_mmio_map ABI change

**RFC ID:** 016  
**Status:** Implemented (v0.1.0)
**Affects:** `fjell-cap/src/rights.rs`, kernel syscall, fjell-init

## Problem (RB-04 from v0.0.10 review)

`sys_mmio_map(phys_addr, size)` accepts any non-RAM address from any task.
A compromised or buggy driver can map UART, CLINT, PLIC, or virtio from any task.

## Proposed fix

### New CapKind

```rust
CapKind::MmioRegion   // authorises access to a specific MMIO physical range
```

### Capability object

```rust
// Stored as object_id → index into a static MmioRegionTable
pub struct MmioRegionObject {
    pub base: usize,
    pub size: usize,
    pub description: [u8; 16],
}
```

### New ABI

```
sys_mmio_map(a0=mmio_cap_handle, a1=offset_within_region, a2=size) -> (status, user_va)
```

The cap's `object_id` identifies the region; `offset + size` is bounds-checked against it.

### Kernel init

At boot, the kernel creates `MmioRegion` caps for each entry in `MMIO_REGIONS` and
installs them in init's CSpace (slots 31–35).

## Defer condition

Requires MmioRegionTable + new kernel objects.  Deferred to M8 when devmgr is
implemented and can distribute device caps.
