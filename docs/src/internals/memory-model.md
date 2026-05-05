# Memory Model

> Implemented in **M2**.  This page describes the target design.

## Physical memory management

Fjell OS uses a two-stage physical allocator to avoid a general kernel heap.

### Stage 1 — `BootAllocator` (bump allocator)

Used only during kernel initialisation.  Allocates forward from
`__kernel_end` toward a fixed upper bound.  Provides frames for:

- The initial kernel page table
- The `FrameAllocator` bitmap
- The task table and scheduler queue storage
- The audit ring buffer

`BootAllocator` never frees.  All regions it hands out are marked
`FrameOwner::ReservedBoot` in the `FrameAllocator`.

### Stage 2 — `FrameAllocator` (bitmap, order-0 only)

Tracks every 4 KiB physical frame as one bit (free/used) plus an optional
`FrameOwner` tag for debugging and audit.  Allocation uses next-fit scan
with a `next_hint` pointer.

**Why not buddy?** M2 allocates only 4 KiB frames and page-table pages.
High-order contiguous allocations are not needed; buddy would add complexity
with no benefit.

## Invariants

| ID | Invariant |
|---|---|
| MM-PHY-001 | Every frame is `Free` or has exactly one `FrameOwner`. |
| MM-PHY-002 | Kernel image frames are never freed. |
| MM-PHY-003 | DTB region is not overwritten after page-table construction. |
| MM-PHY-004 | MMIO frames are never returned by `alloc_frame`. |
| MM-PHY-005 | `free_frame` succeeds only for currently-allocated frames. |

## Virtual memory — Sv39

| Region | Virtual address | Permissions |
|---|---|---|
| Null guard | `0x0000_0000_0000_0000–0xFFFF` | unmapped |
| User text | `0x0000_0000_0010_0000–` | R·X·U |
| User data | follows user text | R·W·U |
| User stack | top = `0x0000_0000_8000_0000` (grows down) | R·W·U |
| Stack guard | one page below stack bottom | unmapped |
| Kernel shared | `0xFFFF_FFC0_0000_0000–` | R·W·X (supervisor) |
| Kernel MMIO | high-half platform window | R·W (supervisor, no X) |

### Invariants

| ID | Invariant |
|---|---|
| MM-VM-001 | No kernel page is mapped U=1 in any address space. |
| MM-VM-002 | The kernel shared map is identical across all address spaces. |
| MM-VM-003 | User text has no W bit. |
| MM-VM-004 | Guard pages remain unmapped. |
| MM-VM-005 | `map_page` does not silently overwrite an existing mapping. |
| MM-VM-007 | `satp` writes are always followed by `sfence.vma`. |
