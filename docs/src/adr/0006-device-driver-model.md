# ADR 0006: User-space driver model and MMIO/DMA capability boundary

**Status:** Superseded — see [ADR-0006 User-Space Driver Model](./0006-user-space-driver-model.md) (RFC 045 rename)  
**Date:** 2026-05-12  
**Milestone:** M6

---

## Context

M6 introduced the first real device driver in Fjell OS: a virtio-mmio block device
driver.  The design document specifies that device drivers must run in user space and
receive only the capabilities they need (DEV-001 through DEV-006).

Three kernel primitives were required:

1. `sys_platform_info_get` — discover the physical address of the virtio-mmio block device
2. `sys_mmio_map` — map a physical MMIO range into the calling task's address space
3. `sys_dma_alloc` — allocate physically-contiguous memory for DMA virtqueue buffers

Implementation encountered several challenges:

- **QEMU legacy virtio (version 1):** QEMU virt's virtio-mmio uses the legacy interface
  (`version=1`), not the modern `version=2` interface.  Register layout, feature
  negotiation, and queue setup differ between versions.
- **DMA physical address alignment:** The DMA buffer must be 4 KiB-aligned for
  `QueuePFN` to be correct (device PA = base + ring offsets).
- **DMA user-space mapping:** DMA frames allocated from kernel RAM (0x80000000+)
  cannot be mapped at their physical address in user tasks because the kernel shares
  L2 entry 2 (VPN[2]=2) with all tasks via `clone_kernel_half`.  Adding the U bit to
  pages in that range would modify shared page table pages.
- **MMIO kernel RAM overlap:** `sys_mmio_map` with no bounds check could map kernel
  text/data into user space (RFC 005 fixed this).

---

## Decision

### sys_platform_info_get

The kernel scans all 8 virtio-mmio slots (0x10001000–0x10008000) at syscall time to
find the device with `magic=0x74726976`, `device_id=2` (block).  The scan runs with
the calling task's satp, so all 8 slots are pre-mapped kernel-accessible (R|W, no U)
in every task's page table from `spawn.rs`.

**Known limitation:** Scanning in the kernel and pre-mapping MMIO in all tasks is a
violation of the kernel mechanism-only principle.  A proper implementation would have
the kernel expose a raw DTB pointer and let `devmgr` perform device discovery.  This
is tracked as future work.

### sys_mmio_map

Maps the requested physical range (page-aligned) into the calling task's page table
with R|W|U permissions.  Uses `remap_page` to allow upgrading pre-existing
kernel-accessible (R|W) mappings to user-accessible (R|W|U).

**RFC 005 security fix:** Requests overlapping `RAM_BASE..RAM_END` are rejected
unconditionally to prevent user-space from mapping kernel memory.

**RFC 004 capability gate:** Caller must hold `CapKind::TaskCreate` or similar device
capability.  Currently gated by bootstrap TaskCreate; a proper `MmioRegion` capability
is deferred to M8.

### sys_dma_alloc (RFC 007)

DMA frames are allocated from the kernel frame allocator and mapped at user VA
`0x60000000+` (VPN[2]=1).  VPN[2]=1 is NOT shared via `clone_kernel_half` (which only
copies VPN[2]=2), so `map_page` succeeds without `AlreadyMapped` collision.

The physical address (kernel RAM PA) is returned as `device_pa`; the device uses it
directly since QEMU has no IOMMU.

**Earlier rejected approach:** A static 16 KiB `DMA_BUF` with `#[repr(align(4096))]`
was used in M6 as a workaround for the shared L2 conflict.  This was replaced by the
per-task VA allocator in RFC 007.

### virtio-blk driver

All virtio-blk initialization and I/O is driven inline from `fjell-init` in M6/M7
(not from the `fjell-driver-virtio-blk` binary, which is a stub).  This is a
deliberate smoke-test simplification; the stub exists to reserve the ImageId and will
be implemented as a real IPC service in M8.

---

## Consequences

- Device drivers can access MMIO after receiving a mapped VA from `sys_mmio_map`.
- DMA memory is usable by user-space drivers and the device simultaneously (VA for CPU,
  PA for device).
- The singleton static `DMA_BUF` (M6) is replaced by a per-task VA bump allocator
  (RFC 007, M7.1), allowing multiple concurrent DMA users.
- `sys_platform_info_get` will be replaced by DTB handoff in M8.
- Full capability-based MMIO gating is deferred to M8 (`MmioRegion` capability).
