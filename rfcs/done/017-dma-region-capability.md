# RFC 017: DmaRegion capability + sys_dma_alloc ownership

**RFC ID:** 017  
**Status:** Implemented (v0.1.0)
**Affects:** `fjell-cap/src/rights.rs`, kernel syscall

## Problem (RB-05 from v0.0.10 review)

`sys_dma_alloc` (RFC 007, per-task VA at 0x6000_0000+):
- Requires no capability
- Tracks no ownership (any task can call)
- Does not zeroize or reclaim on task exit
- Returns only first_pa for multi-page allocation (physically non-contiguous)
- Does not rollback partial allocation on failure

## Proposed fix

### New CapKind

```rust
CapKind::DmaAlloc   // authorises sys_dma_alloc for a specific device
```

### Per-task DmaRegion tracking

```rust
struct DmaRegion {
    owner:    TaskId,
    user_va:  usize,
    pages:    [PhysFrame; 8],  // max 8 pages
    n_pages:  usize,
}
static DMA_REGIONS: ... // static table, MAX_DMA_REGIONS per task
```

### Rollback on partial failure

If `alloc_frame` fails mid-loop, free already-allocated frames and return `NoMemory`.

### Zeroize on task exit

On `TaskState::Exited`, walk the task's DmaRegion list, unmap user VAs, zeroize frames,
free frames.

### Contiguous DMA note

For virtio, a single 4 KiB page holds all virtqueue structures (descriptors, avail,
used, header, data, status).  Multi-page allocations may not need physical contiguity
for current use.  Document this limitation.

## Defer condition

Requires DmaRegion tracking table.  Deferred to M8.
