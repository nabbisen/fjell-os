# Fjell OS v0.1 — MMIO / DMA Boundary Audit

**Version:** v0.1.3.  
**Produced by:** RFC 030 (also known as RFC-v0.1.x-007).  
**Input contract to:** RFC 035 (MmioRegion ABI), RFC 036
(DmaRegion zeroize/quarantine).

---

## 1. MMIO Current Model (v0.1.0 / v0.1.2)

`sys_mmio_map` (syscall nr 90) accepts a caller-supplied raw physical
address and size:

```
mmio_map(phys_addr: usize, size: usize) -> SysError
```

The caller then accesses the mapped virtual address directly.

### What exists

- A RAM-exclusion guard (RFC 005) rejects any physical address below
  a hard-coded threshold (typically 0x8000_0000 on QEMU virt). This
  prevents mapping kernel or task RAM as a device register window.
- The mapping is restricted to the calling task's address space
  (process-local mapping).
- The mapping is non-executable (PTE_X is not set, per RFC 009).

### What does NOT exist

- No `MmioRegionObject`. There is no kernel-tracked region object.
- No `MmioRegion` capability. Any task that can invoke `sys_mmio_map`
  can map any non-RAM physical range.
- No offset / size validation against a pre-authorised region.
- No lease binding. Revoking any existing capability has no effect
  on a granted MMIO mapping.
- No owner-task tracking at the region level.
- No enforcement that mappings are user-accessible only (PTE_U is set
  by default but not explicitly asserted per-region).

---

## 2. DMA Current Model (v0.1.0 / v0.1.2)

DMA allocation (`sys_dma_alloc`, nr 110) allocates one or more
physical pages and maps them into the caller's address space.

### What exists

- Per-task DMA allocator introduced in RFC 007.
- `dma_revoke` (nr 112) exists; marks the region Revoked per task.
- Owner-task tracking is implicit (the allocating task's table).
- `dma_share` (nr 111) exists as a stub.

### What does NOT exist

- No `DmaRegionObject` with a 4-state cleanup machine
  (Active → Revoked → Quarantined → Zeroized → Freed).
- No `DmaRegion` capability. Any task can call `dma_alloc`.
- No device-id tracking. A region cannot be attributed to a specific
  device for quarantine purposes.
- No lease binding. DMA revoke does not connect to the lease system.
- No deterministic zeroize on revoke or task exit. The page may be
  returned to the allocator without zeroing.
- No quarantine timeout. A stuck device can hold the frame
  indefinitely.
- No `MAX_DMA_FRAMES = 1` enforcement. Multi-page DMA is possible,
  which makes physical-contiguity guarantees implicit and unverified.

---

## 3. Threat Analysis

### T-MMIO-001: Arbitrary device register access

Any task that can reach `sys_mmio_map` can map any non-RAM physical
range. An attacker service can:

- Map the PLIC to manipulate interrupt priorities.
- Map a UART to sniff/inject serial output.
- Map a virtio device config space and reprogram it.

**Mitigations today:** Only services started by `fjell-init` can
invoke `sys_mmio_map` (IPC-controlled access to `fjell-devmgr`).
This is a policy-level barrier, not a kernel capability boundary.

**v0.2 fix:** RFC 035 — capability-bound `MmioRegion` object;
the kernel validates kind + rights + lease + offset + size + non-RAM
before mapping.

### T-MMIO-002: Executable MMIO mapping

If `PTE_X` were accidentally set, a device that can inject data
could be used to execute arbitrary code.

**Status today:** RFC 009 removed `PTE_X` from MMIO mappings.
Defense-in-depth; the specific register-programming attack described
in T-MMIO-001 does not require execute.

### T-DMA-001: DMA page reuse before zeroize

On task exit or capability revoke, the DMA frame may be returned to
the page allocator without zeroing. The next task that allocates a
page gets the previous task's data.

**Status today:** No explicit zeroize. Frame return path clears the
struct but does not wipe the physical page contents.

**v0.2 fix:** RFC 036 — deterministic zeroize on every DMA revoke,
with quarantine timeout to prevent device-induced stall.

### T-DMA-002: Unbounded DMA quarantine

A misbehaving device can prevent DMA cleanup indefinitely, causing
the system to exhaust physical frames.

**Status today:** No quarantine timeout.

**v0.2 fix:** RFC 036 — DMA quarantine with devmgr timeout budget.

### T-DMA-003: Multi-page DMA leaks

A multi-page DMA region cannot guarantee physical contiguity without
a buddy allocator or explicit scatter-gather metadata. If pages are
not adjacent, the device may interpret the mapping incorrectly,
potentially reading/writing data from adjacent allocations.

**Status today:** No `MAX_DMA_FRAMES` enforcement.

**v0.2 fix:** RFC 036 — 1-page maximum restriction; multi-page
scatter-gather deferred to later milestone.

---

## 4. Current Mitigations

| Mitigation | RFC | Effective? |
|---|---|---|
| RAM-exclusion guard on `mmio_map` | RFC 005 | Partial — coarse threshold only |
| Non-executable MMIO mappings | RFC 009 | Yes — defense-in-depth |
| Per-task DMA allocator | RFC 007 | Partial — no cap enforcement |
| MMIO access via devmgr IPC | ADR-0006 | Policy-level only |

---

## 5. Known Gaps

| Gap | Severity | v0.2 Closing RFC |
|---|---|---|
| `sys_mmio_map` has no capability check | Release Blocker | RFC 035 |
| No `MmioRegionObject` or region table | Release Blocker | RFC 035 |
| `sys_dma_alloc` has no capability check | Release Blocker | RFC 036 |
| No DMA zeroize on revoke / task exit | Release Blocker | RFC 036 |
| No DMA quarantine timeout | Release Blocker | RFC 036 |
| No 1-page DMA enforcement | Release Blocker | RFC 036 |
| No device-id tracking on DMA regions | Release Blocker | RFC 036 |
| No lease binding on MMIO or DMA caps | Release Blocker | RFC 033 + 035 + 036 |

All six gaps are Release Blockers for v0.2.0.

---

## 6. Required v0.2 Fixes

```
- MmioRegion capability (CapKind::MmioRegion + MMIO_MAP right)
- MmioRegionObject with id, owner, phys_base, length, lease
- Static MmioRegionTable (interim; replaces DTB-driven discovery)
- sys_mmio_map(mmio_region_cap, offset, size) — breaks v0.1.x ABI
- Offset + size bounds check within region
- Non-RAM enforcement (defense-in-depth carried forward)
- DmaRegion capability (CapKind::DmaRegion)
- DmaRegionObject with owner, device, lease, state machine
- MAX_DMA_FRAMES = 1 restriction
- Active→Revoked→Quarantined→Zeroized→Freed state machine
- Deterministic zeroize on revoke or task exit
- Quarantine timeout (devmgr reset path)
- Device-id tracking on DmaRegion
- Lease binding on both MmioRegion and DmaRegion capabilities
```

---

## 7. Negative Tests Required

Per RFC 026 (category `mmio`) and RFC 026 (category `dma`):

```
NEG:MMIO:MAP_WITHOUT_CAP:PASS
NEG:MMIO:MAP_RAM_REJECTED:PASS
NEG:MMIO:OFFSET_OUT_OF_RANGE:PASS
NEG:MMIO:REVOKED_REGION_REJECTED:PASS

NEG:DMA:ALLOC_WITHOUT_CAP:PASS
NEG:DMA:SIZE_TOO_LARGE:PASS
NEG:DMA:REVOKED_REGION_REJECTED:PASS
NEG:DMA:ZEROIZED_ON_EXIT:PASS
NEG:DMA:QUARANTINE_TIMEOUT:PASS
NEG:DMA:QUARANTINED_PAGE_NOT_REUSED:PASS
```

All are `DEFERRED` at v0.1.2 (enforcement lands in v0.2).

---

## 8. Deferred Work

| Item | Deferred to |
|---|---|
| IOMMU integration | v0.3 |
| Multi-page scatter-gather DMA | When a device requiring it ships |
| DTB-driven MmioRegionTable | v0.3 |
| MMIO region sharing between tasks | v0.3 or later |
| DMA region sharing / pass-through | v0.3 or later |
