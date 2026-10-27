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

Tracks every 4 KiB physical frame as one bit (free/used) plus a `FrameOwner`
tag for debugging and audit. The kernel builds it **with** the owner array
(RFC-0.39-003): `owner_of` names who holds any frame, which is what a memory
defect needs and what an unowned allocator cannot give.  Allocation uses next-fit scan
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
| MM-PHY-006 | **No frame from `RAM_BASE` to the top of the kernel stack (`__stack_top`) is ever free.** The 4 MiB kernel stack is BSS, and trap entry resets `sp` to its top; a free frame there is handed to a task and the kernel stack then overwrites it. This was false until RFC-0.39-003 (only the first 2 MiB of the stack was reserved, by the coincidence that the unused boot-allocator window began where the stack does — E-097). `kmain` now reserves the stack as `KernelStack`, starts the boot window after it, and **stops the boot** with the offending frame if the invariant does not hold. |
| MM-PHY-007 | A DMA frame is zeroed and freed only if the allocator records it as held by the releasing task; otherwise it is left allocated and the console says so (RFC-0.39-003, E-098). |

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
