# RFC 007: Per-task DMA allocator (replace DMA_BUF singleton)

**RFC ID:** 007  
**Status:** Implemented  
_(was: Accepted, deferred to M7.1)  
**Affects:** `crates/fjell-kernel/src/main.rs`, `trap/syscall.rs`

## Problem (RB-03)

`sys_dma_alloc` always returns the same `DMA_BUF` static buffer.
Multiple callers alias the same physical memory.  Size argument is ignored.
The buffer is only mapped in init's page table.

## Proposed fix

1. Remove `DMA_BUF` static.
2. `sys_dma_alloc(size)`:
   - Allocate `ceil(size / PAGE_SIZE)` frames from the frame allocator.
   - Track them in a per-task `DmaRegion` list (max 4 regions per task for M7.1).
   - Map frames at a task-local VA base: `DMA_VA_BASE + task_id * DMA_SLOT_SIZE`.
   - Return `(user_va, device_pa)` where `user_va != device_pa` is acceptable.
3. On task exit: unmap and free all DMA regions, zeroize frames.

## Key constraint

The root cause of the original `AlreadyMapped` failure was mapping PA in kernel-half
VPN[2]=2.  Fix: allocate from a region-specific PA range, OR use a user-VA that does
not conflict with the shared kernel half.  Recommended: use VA `0x6000_0000` as DMA
base, well below `0x8000_0000`.

## Defer condition

Implement after RFC 005 (MMIO safety) and RFC 004 (capability gating).
