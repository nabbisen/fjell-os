# RFC 052: DMA region-table-full rollback and cap-based revoke

**RFC ID:** 052
**Also known as:** RFC-v0.2-018
**Status:** Proposed
**Target version:** v0.2.11
**Phase:** MMIO/DMA/audit hardening
**Closes review items:** RB-08, RB-09 (and resolves H-03 by explicit deferral)
**Depends on:** RFC 036 (DMA region capability), RFC 048 (handle-based require_cap)

## Problem

### RB-08: DMA region-table allocation result ignored

`crates/fjell-kernel/src/main.rs:177-193` defines
`DmaRegionTable::alloc(owner, user_va, frame_pa) -> bool` which returns
`false` when the table is full.

`crates/fjell-kernel/src/trap/syscall.rs:708-713` calls:

```rust
crate::dma_table().alloc(task_id, user_va_start, first_pa);

tf.gpr[REG_A0] = 0;
tf.gpr[REG_A1] = user_va_start;
tf.gpr[12]     = first_pa;
```

The return value is **dropped**.  When the table fills up:

- The user mapping has already been created (frames allocated, page table
  updated)
- `alloc()` silently returns `false`
- The syscall returns `Ok` with valid-looking VA and PA
- The DMA region is **not tracked** — `release_task` and explicit revoke
  will not zeroize it

This is exactly the class of DMA leak that RFC 036 was written to prevent.

### RB-09: DMA revoke takes raw PA, not a capability

`crates/fjell-kernel/src/trap/syscall.rs:716-730` defines:

```
sys_dma_revoke(a0 = device_pa)
```

It revokes by physical address, validated only against the caller's
ownership in the region table.  It does not:

- Require a `DmaRegion` capability
- Check `DMA_REVOKE` right
- Check `ObjectScope::DmaRegion(region_id)`
- Check lease epoch
- **Unmap the user VA** (the page table entry remains, pointing at a freed
  PA — a reused-after-free hazard)

`crates/fjell-kernel/src/main.rs:146-149` explicitly marks `user_va` as
future-unmap work.  In v0.2.7 we shipped `NEG:DMA:ZEROIZE_ON_EXIT:PASS`
that relies on the cooperative scheduler to verify the zero — that test
is valid for what it claims (zeroize), but it does not prove the page
table was updated.

### H-03: Quarantine timeout

RFC 036 §"Quarantine" describes a deferred quarantine state — a frame is
zeroized but held out of the allocator for some interval before being
recycled.  This is not implemented; frames return to the allocator
immediately.  The v0.2.0 release-gate document does not list a marker for
quarantine, but the review (H-03) asked for explicit confirmation.

## Proposed fix

### Part A — RB-08: rollback on table full

```rust
// sys_dma_alloc — replaces lines 708-713.
if !crate::dma_table().alloc(task_id, user_va_start, first_pa) {
    // Rollback: unmap, zeroize, free.
    unmap_dma_range(tidx, user_va_start, page_count);
    unsafe { core::ptr::write_bytes(first_pa as *mut u8, 0, page_count * 4096); }
    for i in 0..page_count {
        let pa = PhysAddr(first_pa + i * 4096);
        if let Ok(frame) = PhysFrame::from_pa(pa.0) {
            unsafe { (*fa).free_frame(frame); }
        }
    }
    err(tf, SysError::ResourceExhausted);
    return;
}
```

`unmap_dma_range` is a new internal helper in `fjell-kernel/src/mm/` that
walks the page table for `tidx` and clears the entries in the user VA
range.  It is the same primitive needed by Part B below — shared
implementation.

### Part B — RB-09: cap-based revoke with unmap

New ABI:

```
sys_dma_revoke(cap_handle)
```

Replaces the raw-PA variant.  Validation (via `require_cap_on`, RFC 049):

1. Look up cap by handle (generation-validated)
2. Kind == `DmaRegion`
3. Rights include `DMA_REVOKE`
4. Scope `Any` or `DmaRegion(region_id)`
5. Lease active

If valid, the kernel:

1. Finds the `DmaRegionEntry` by `region_id` (the cap's `object_id`)
2. Unmaps the user VA range via `unmap_dma_range`
3. Zeroizes the physical frame (`write_bytes(pa, 0, 4096)`)
4. Frees the frame (`free_frame`) — or quarantines if Part C lands
5. Clears the table entry (`DmaRegionEntry::free`)
6. Returns `Ok`

`DmaRegionEntry::user_va` is now read on revoke (was `dead_code` per
RB-09 evidence).

The audit record:
- `AuditKindInternal::DmaRevoke` (already exists)
- `arg0 = region_id`
- `arg1 = device_pa`
- `result = 0`

### Part C — H-03: quarantine deferral statement

This RFC **does not implement** quarantine.  It documents the deferral
explicitly:

> Quarantine timeout (RFC 036) is deferred to v0.3.  v0.2 implements
> synchronous zeroize-on-revoke and zeroize-on-task-exit.  The
> `NEG:DMA:QUARANTINE_TIMEOUT` marker (mentioned in RFC 036's optional
> markers list) is not part of the v0.2 release-gate matrix.

This closes H-03 by being explicit rather than implementing.

### `DmaRegionEntry` changes

```rust
pub struct DmaRegionEntry {
    pub region_id:   u32,    // NEW — referenced by cap.object_id
    pub state:       DmaRegionState,
    pub owner_tid:   TaskId,
    pub user_va:     usize,   // already exists; now read on revoke
    pub frame_pa:    usize,
    pub size_pages:  u16,
    pub lease_id:    Option<LeaseId>,  // already exists
}
```

`region_id` becomes the index into the table (or an explicit u32 if the
table is sparse).  Caps to DMA regions carry this id in `object_id`.

### `sys_dma_alloc` returns the region cap

After successful alloc, the kernel installs a `DmaRegion` cap into the
caller's CSpace with:

- `kind = DmaRegion`
- `object_id = region_id`
- `rights = DMA_ALLOC | DMA_USE | DMA_REVOKE` (default; cap-broker may
  later narrow this when delegating)
- `scope = DmaRegion(region_id)`
- `lease = Some(LeaseBinding { lease_id, epoch_at_issue: current })` —
  bound to a fresh per-region lease

The cap handle is returned in `a2`.  Previous ABI:

```
a0 = status
a1 = user_va
a2 = device_pa (for legacy callers; deprecated in favor of cap handle)
```

New ABI:

```
a0 = status
a1 = user_va
a2 = cap_handle (slot+generation, packed)
a3 = device_pa (for drivers that need to program devices)
```

Callers that issue `sys_dma_revoke` use the cap handle from `a2`, not
the raw PA.

## Rationale

**Why fail with `ResourceExhausted`, not `NoMemory`?**  `NoMemory` implies
the frame allocator failed; `ResourceExhausted` implies a different
resource (the tracking table) is full.  Both are recoverable for the
caller — the distinction aids diagnosis.

**Why unmap before free?**  If we free before unmap, another task could
allocate the same frame and the original task's stale page-table entry
would alias it.  Unmap-before-free is the standard order.

**Why per-region lease (and not one lease per task)?**  Per-region leases
let cap-broker / driver-manager revoke a specific DMA region without
affecting other regions of the same task.  RFC 036 already specifies this
direction; this RFC realizes it.

**Why not implement quarantine in v0.2?**  Quarantine needs a wall-clock
timer interrupt to release frames.  The timer infrastructure exists but
exposing it to the DMA allocator adds a kernel-internal subscriber path
that is larger than v0.2 scope.  Synchronous zeroize provides the same
end-state security property (no data leaks).  Quarantine is a
performance/availability optimization, not a security one.

**Why pass cap handle in `a2` (not replacing `a1` user_va)?**  Drivers
need both VA (to access the memory) and cap handle (to revoke).  The PA
in `a3` is for devices that need the physical address (DMA engines).
The legacy single-return ABI cannot serve all three needs.

## Impact

### Crates affected

| Crate | Change |
|-------|--------|
| `fjell-kernel/main.rs` | `DmaRegionEntry::region_id` field, table indexing |
| `fjell-kernel/mm/` | `unmap_dma_range` helper |
| `fjell-kernel/trap/syscall.rs` | `sys_dma_alloc` rollback + cap install; `sys_dma_revoke` rewrite |
| `fjell-syscall` | `sys_dma_alloc` returns `(user_va, cap_handle, device_pa)`; `sys_dma_revoke(cap_handle)` |
| `fjell-cap` | (no change — `ObjectScope::DmaRegion` already exists) |
| `fjell-neg-test` | Updated to use cap handle |
| Driver crates | When DMA-using drivers land, they take cap handle |

### Backward compatibility

Breaking ABI change for DMA syscalls — same hardening-line policy as
RFC 048.  Only neg-test uses these calls today.

### Audit trail

`AuditKindInternal::DmaAlloc` records now carry `region_id` in `arg0`
(was previously `0`).  `DmaRevoke` records carry `region_id` in `arg0`
(was previously the raw PA — now in `arg1`).

## Test plan

### Host (unit tests in `fjell-kernel::dma_table`)

1. `DmaRegionTable::alloc` returns false when full
2. After Part A rollback, the table state is unchanged from pre-alloc
3. `DmaRegionTable::find_by_id(region_id)` resolves an active region

### Host (unit tests in `fjell-cap`)

4. `ObjectScope::DmaRegion(7).matches_target(7)` → true
5. `ObjectScope::DmaRegion(7).matches_target(8)` → false

### QEMU

6. `NEG:DMA:RIGHTS_CHECK:PASS` — unchanged (already exists)
7. `NEG:DMA:REVOKE_EXPLICIT:PASS` — now uses cap handle; same outcome
8. `NEG:DMA:ZEROIZE_ON_EXIT:PASS` — unchanged
9. **NEW** `NEG:DMA:REGION_TABLE_FULL_ROLLBACK:PASS` — neg-test fills the
   region table (current capacity), the next `sys_dma_alloc` returns
   `ResourceExhausted` and the user VA is **not** mapped (verified by
   attempting a read → page fault → recovered).
10. **NEW** `NEG:DMA:REVOKE_WITHOUT_RIGHT_REJECTED:PASS` — neg-test mints
    a DMA cap with `DMA_USE` only (no `DMA_REVOKE`), calls `sys_dma_revoke`,
    receives `PermissionDenied`.
11. **NEW** `NEG:DMA:REVOKE_UNMAPS_VA:PASS` — after revoke, reading from
    the previously-mapped user VA produces a page fault (verified by
    spawning a sub-task that does the read and observing `Faulted`
    status — uses the same machinery as `NEG:SVC:FAULT_DETECTED`).

Marker #11 is the closing test for RB-09's unmap requirement.  The current
v0.2.7 `ZEROIZE_ON_EXIT` test reads from the VA after revoke and gets
zeros — that test must be updated to read from a *fresh* VA (post-revoke
the VA should fault, not read zeros).

## Implementation notes

- `unmap_dma_range` shares page-table-walking code with the (forthcoming)
  `unmap_mmio_range` from RFC 051.  Consider a common
  `mm::unmap_page_range(tidx, va, page_count)` primitive.
- The DMA region table capacity is currently `MAX_DMA_REGIONS = 64` (per
  `fjell-kernel/main.rs`).  Marker #9 needs neg-test to exhaust this —
  it can call `sys_dma_alloc(4096)` in a loop and stop at the first
  `ResourceExhausted`.
- After marker #11's introduction, the `NEG:DMA:ZEROIZE_ON_EXIT:PASS`
  test must do one of:
  - Verify zeroize *before* revoke (write pattern, observe zero via
    explicit kernel call — not possible from user space, drop this)
  - Verify zeroize via task-exit path (spawn sub-task with DMA cap, let
    it exit, then read its old frame — but its old frame is now
    reallocated/zeroed)
  - **Recommended:** verify zeroize indirectly: alloc, write pattern,
    revoke (frame now zeroed and freed), alloc again (likely same
    frame), read → bytes are zero.  This is more reliable than the
    current "read freed VA" approach which becomes a fault after RB-09.

## Open questions

- Should `sys_dma_revoke` also revoke the lease, or just clear the region
  entry?  Recommendation: do both — revoking the lease causes any cap
  copies (RFC 049 with COPY right) to fail their next use too.
- Should the cap-broker mint per-region DMA caps as part of a
  `RequestDma(size_bytes)` protocol message?  Recommendation: yes, but
  that's RFC 056 scope.  v0.2.11 keeps DMA caps installed by the kernel
  on successful `sys_dma_alloc`.
