# RFC 051: Device VMA range and MMIO mapping correctness

**RFC ID:** 051
**Also known as:** RFC-v0.2-017
**Status:** Implemented
**Target version:** v0.2.11
**Phase:** MMIO/DMA/audit hardening
**Closes review items:** RB-07, H-05
**Depends on:** RFC 035 (MmioRegion capability ABI), RFC 048 (handle-based require_cap)

## Problem

`crates/fjell-kernel/src/trap/syscall.rs:538-618` implements `sys_mmio_map`.
The ABI is capability-based (RFC 035) but the mapping itself has three
defects:

```rust
let mut va = phys_addr;
// ...
let user_va = va;
let _ = remap_page(... VirtAddr(user_va), ...);
```

**Defect 1 — PA as VA.**  The user virtual address is taken from the
physical address.  For QEMU virt's PLIC at PA `0x0C00_0000`, this places a
user mapping at VA `0x0C00_0000`.  This collides with any future user
mapping in that range and gives every service the same VA layout for the
same device (an information leak surface).

**Defect 2 — `remap_page` result ignored.**  If `remap_page` fails (out of
page-table memory) or overwrites an existing user mapping, `sys_mmio_map`
still returns `Ok`.  Driver code believes it has the device mapped when it
does not, or worse, the kernel silently replaced one of the driver's other
mappings.

**Defect 3 — scope passes `None`.**  The `require_cap` call passes
`ObjectScope::Any` (effectively), so an MMIO cap scoped to region 3 can
map region 7.  RFC 035 §3 requires scope enforcement.

Companion issue (**H-05**): `crates/fjell-kernel/src/task/spawn.rs:152-159`
installs MmioRegion caps for **every region into every service**, including
those that have no need for them.  This bypasses the cap-broker policy
(RFC 040) for MMIO authority.

## Proposed fix

### Reserved device VMA range

Add a constant to `crates/fjell-kernel/src/platform/qemu_virt.rs`:

```rust
/// Per-task device-VMA window for MMIO mappings.
/// 256 MiB, immediately below the kernel base.  Each task gets a private
/// view into this range; the kernel allocator hands out 4 KiB-aligned VAs.
pub const DEVICE_VMA_BASE: usize = 0x7000_0000;
pub const DEVICE_VMA_SIZE: usize = 0x1000_0000;  // 256 MiB
pub const DEVICE_VMA_END:  usize = DEVICE_VMA_BASE + DEVICE_VMA_SIZE;
```

`DEVICE_VMA_BASE..DEVICE_VMA_END` does not overlap RAM (`0x8000_0000+`),
does not overlap the existing user heap range, and lies below the kernel.
The synthetic test region 4 (`base=0x7FFE_0000`) is also below this range —
no conflict.

### Per-task device VMA allocator

Each task's `Tcb` gets a small bitmap allocator for its device VMA range:

```rust
pub struct DeviceVmaAllocator {
    bitmap: [u32; 32],  // 1024 4-KiB pages = 4 MiB max per task
    next:   u16,        // hint for next allocation
}
```

API:

```rust
impl DeviceVmaAllocator {
    pub fn alloc(&mut self, page_count: usize) -> Option<VirtAddr>;
    pub fn free(&mut self, va: VirtAddr, page_count: usize);
}
```

Stored in `Tcb` (`crates/fjell-kernel/src/task/tcb.rs`).  Per-task is
correct: services should not see each other's MMIO VA layouts.

### Revised `sys_mmio_map` flow

```rust
pub fn sys_mmio_map(tf, tidx, ct) {
    let cap_h     = CapHandle(tf.gpr[REG_A0] as u32);
    let offset    = tf.gpr[REG_A1];
    let size_bytes = tf.gpr[REG_A2];

    // 1. RFC 049-style: validate cap with scope = MmioRegion(region_id).
    //    We don't yet know region_id — resolve cap first.
    let (region_id, region_base, region_size) = {
        let cs   = ct.cspace(tidx).ok_or(...)?;
        let slot = cs.slot_by_handle(cap_h).map_err(...)?;
        let cap  = slot.cap.as_ref().ok_or(...)?;
        require_cap_on(cap_h, CapKind::MmioRegion, CapRights::MMIO_MAP)?;
        let rid = cap.object_id;
        let region = MMIO_TABLE.get(rid).ok_or(InvalidArg)?;
        (rid, region.base, region.size)
    };

    // 2. Scope check (RFC 051 mandatory):
    //    cap.scope must be Any or MmioRegion(rid).
    //    Done inside require_cap_on via the cap's scope field.

    // 3. Bounds check (existing RFC 035).
    if offset + size_bytes > region_size { err(InvalidArg); return; }

    // 4. RAM-guard check (existing RFC 005).
    let phys_addr = region_base + offset;
    let end_pa = phys_addr + size_bytes;
    if phys_addr < RAM_END && end_pa > RAM_BASE { err(InvalidArg); return; }

    // 5. Allocate device VMA range.
    let page_count = (size_bytes + 4095) / 4096;
    let user_va = match dev_vma.alloc(page_count) {
        Some(va) => va,
        None     => { err(ResourceExhausted); return; }
    };

    // 6. Map each page, checking every result.  Rollback on failure.
    for i in 0..page_count {
        let pa = PhysAddr(phys_addr + i * 4096);
        let va = VirtAddr(user_va.0 + i * 4096);
        match remap_page(va, pa, MMIO_FLAGS) {
            Ok(())                       => continue,
            Err(MapError::AlreadyMapped) => {
                rollback_pages(user_va, i);
                dev_vma.free(user_va, page_count);
                err(InvalidArg); return;
            }
            Err(_)                       => {
                rollback_pages(user_va, i);
                dev_vma.free(user_va, page_count);
                err(InternalError); return;
            }
        }
    }

    // 7. Success: return the allocated user VA.
    tf.gpr[REG_A0] = SysError::Ok as isize as usize;
    tf.gpr[REG_A1] = user_va.0;
}
```

`rollback_pages` unmaps the partial mappings made in steps 1..i.

### Narrower MMIO grants at spawn time (closes H-05)

`crates/fjell-kernel/src/task/spawn.rs` currently installs all 5 MMIO
region caps into every service.  Replace with per-service whitelist:

| Service | MMIO regions |
|---------|--------------|
| `fjell-driver-virtio-blk` | 3 (virtio-mmio) |
| `fjell-devmgr` | 2 (PLIC) |
| `fjell-storaged` | (none — uses virtio-blk driver via IPC) |
| `fjell-neg-test` | 0, 1, 2, 3, 4 (test requires breadth, including the RAM-guard test region) |
| (all other services) | (none) |

The whitelist lives in `crates/fjell-kernel/src/task/spawn.rs` as a small
`fn mmio_grants_for(image_id) -> &'static [u32]`.  No cap-broker change
needed yet (cap-broker MMIO delegation is RFC 056 scope).

## Rationale

**Why a reserved VA range and not per-driver custom VA?**  Drivers should
not need to know what VA they will get — the kernel allocates.  Reserving
a single range simplifies the page-table layout and makes leaks visible
(everything in `0x7000_0000..0x8000_0000` is by definition a device
mapping).

**Why 256 MiB?**  The PLIC alone is 64 MiB.  256 MiB allows the four
existing regions plus the synthetic test region plus headroom for future
hardware.

**Why per-task allocator?**  Two reasons.  (1) Services should not see
identical VAs for the same device; identical VAs make ROP-style exploits
cross-service.  (2) Per-task bitmap is small (128 bytes) — cheap to
include in TCB.

**Why bitmap and not free-list?**  4 MiB / 4 KiB = 1024 pages per task,
fits in 32 × u32 = 128 bytes.  Bitmap with a hint pointer is O(1) common
case, O(n) worst case where n=32 — fine for an OS-level allocator on a
single hart.

**Why narrow MMIO grants now?**  RFC 040 cap-broker is the long-term home
for MMIO delegation but RFC 056 (cap-broker installation) is v0.2.12.
Closing H-05 in v0.2.11 unblocks the v0.2 release without waiting for the
broker rework — and confirms the principle that broad pre-granting at
spawn time bypasses policy.

## Impact

### Crates affected

| Crate | Change |
|-------|--------|
| `fjell-kernel/platform/qemu_virt.rs` | `DEVICE_VMA_BASE/SIZE/END` constants |
| `fjell-kernel/mm/` | `DeviceVmaAllocator` struct + tests |
| `fjell-kernel/task/tcb.rs` | `Tcb` gets a `dev_vma: DeviceVmaAllocator` field |
| `fjell-kernel/trap/syscall.rs` | `sys_mmio_map` rewrite (above) |
| `fjell-kernel/task/spawn.rs` | `mmio_grants_for` whitelist (H-05) |
| Driver crates | (no change — they receive `user_va` as before; only the value differs) |
| `fjell-neg-test` | Test no longer assumes PA == VA |

### Backward compatibility

The user VA returned by `sys_mmio_map` is now in `0x7000_0000..0x8000_0000`
instead of equal to the PA.  Drivers using `sys_mmio_map`'s return value
correctly will continue to work.  Drivers that hard-coded PAs would have
been broken anyway — none exist in-tree.

### Audit trail

Failed maps now record one of:
- `InvalidArg` (bounds, RAM guard)
- `PermissionDenied` (cap right, scope)
- `ResourceExhausted` (device VMA exhausted)
- `InternalError` (page-table failure)
- `AlreadyMapped` recorded as `InvalidArg` (rare; indicates allocator bug)

## Test plan

### Host (unit tests in `fjell-kernel::mm::dev_vma`)

1. `DeviceVmaAllocator::new` produces an allocator with all pages free
2. `alloc(1)` returns `DEVICE_VMA_BASE`
3. `alloc(1); alloc(1)` returns two distinct, 4-KiB-spaced VAs
4. `alloc(N)` fails (returns `None`) once all 1024 pages are claimed
5. `free(va, n)` followed by `alloc(n)` returns `va` (or another free range)

### Host (unit tests in `fjell-cap`)

6. `ObjectScope::MmioRegion(3).matches_target(3)` → true
7. `ObjectScope::MmioRegion(3).matches_target(4)` → false (already exists; verify)

### QEMU (new and recharacterized markers)

8. `NEG:MMIO:SCOPE_MISMATCH_REJECTED:PASS` — neg-test holds a cap with
   `scope = MmioRegion(0)`, calls `sys_mmio_map` against region 3, gets
   `PermissionDenied`.  (New marker; requires cap-broker minting narrowed
   caps — or hand-installed via spawn for the test.)
9. `NEG:MMIO:ALREADY_MAPPED_REJECTED:PASS` — neg-test maps a region
   successfully, then maps it again at an overlapping device VA, gets
   `InvalidArg`.  Tests rollback correctness.
10. `NEG:MMIO:DEVICE_VMA_EXHAUSTED:PASS` — neg-test maps 1024 pages
    successfully, the 1025th gets `ResourceExhausted`.

Existing markers (`RIGHTS_CHECK`, `BOUNDS_REJECTED`, `RAM_GUARD_REJECTS`)
continue to fire — they're unrelated to VA assignment.

## Implementation notes

- The `dev_vma` field in `Tcb` is per-task; on task exit (`release_task`)
  it is dropped along with the rest of the TCB.  No explicit cleanup
  needed beyond unmapping pages.
- `rollback_pages` should be a private helper in `sys_mmio_map` — not
  exposed.  It calls `unmap_page` for each VA in the failed range; failure
  here is unrecoverable (`InternalError`).
- The `mmio_grants_for` function should be ordered: most-specific
  services first (drivers), least-privileged services last (`fjell-init`
  gets no MMIO).  Document the layout in code comments.
- The 256-MiB device range is checked against RAM and the test region:

  ```
  DEVICE_VMA_BASE = 0x7000_0000
  DEVICE_VMA_END  = 0x8000_0000  (exclusive)
  test region 4   = 0x7FFE_0000..0x8001_0000  (overlaps DEVICE_VMA_END area)
  ```

  The test region's user mapping (from `sys_mmio_map`) will fall in
  `DEVICE_VMA_BASE..DEVICE_VMA_END`, not at the PA `0x7FFE_0000`.  This is
  fine — the RAM-guard check (step 4 above) uses the PA, not the VA.

## Open questions

- Should `DeviceVmaAllocator` track which `MmioRegion` cap owns each VA
  range?  Useful for later `sys_mmio_unmap`.  Defer — v0.2 has no unmap.
- Should the 256-MiB window be split between MMIO and DMA?  Currently DMA
  uses a separate allocator returning PAs.  Recommendation: keep separate;
  RFC 052 reviews DMA allocator.
