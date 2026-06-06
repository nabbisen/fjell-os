# RFC-v0.7.4-001: DMA Lifetime Safety

**Status.** Implemented (v0.7.1)

## Status

Draft (closes review findings **C-RB-01 (CRITICAL), W-RB-05**)

## Target Version

`v0.7.4`

## Summary

Close the critical DMA isolation gap identified in the crates review:
`sys_dma_revoke` currently zeroizes and frees the physical frame but
does NOT invalidate the user-space PTE that maps it.  This produces a
stale mapping to a freed frame; if the frame allocator later hands
that frame to a different task, the original owner retains a live
virtual mapping to memory now belonging to someone else.  Also
complete the broader DMA tracking gaps (device_id, lease_id,
DmaRegionId, quarantine timeout).

This is the **highest-severity release blocker** identified in either
review.

## Motivation

Crates review §5 RB-01 (verbatim):

```text
DmaRegionEntry stores user_va, but the field is marked as future-use
only.

sys_dma_revoke documents that user VA unmap is deferred to v0.3
(frame is zeroized and freed; VA stays mapped).

The region revoke path zeroizes and frees the physical frame, then
frees the table entry. It does not invalidate the old PTE in the
owning task.
```

The architect labelled the consequences:

```text
- stale mapping to reused memory
- cross-task memory exposure
- corruption of newly allocated frame contents
- broken DMA revoke guarantee
- false sense of v0.2/v0.7 DMA boundary closure
```

Whole-project review §4 RB-05 documents the broader gaps:

```text
The DMA table tracks owner, user_va, frame_pa, state.
It does not yet track device id, lease id, DmaRegionId, quarantine
timeout state.
```

Both findings must close before any service using DMA goes live
(RFC-v0.7.3-001 networking takes a hard dependency on this RFC).

## Goals

```text
- sys_dma_revoke unmaps user_va before freeing the frame.
- sfence.vma is executed after unmap; TLB consistency is guaranteed.
- DMA region is represented by a DmaRegionId (object_id-shaped).
- Owner, device, lease, user_va, frame, state are all tracked.
- Unmap failure quarantines the frame instead of returning it to
  the allocator.
- DMA map failures during sys_dma_alloc do NOT produce a live region.
- Quarantine timeout exists as an active timer-driven mechanism.
- Acceptance tests prove that a revoked DMA region cannot be re-read
  by the original task.
```

## Non-Goals

```text
- No multi-tenant DMA arbitration (single owner per region; v0.8+).
- No IOMMU integration (current QEMU virt doesn't expose one).
- No persistent DMA records across reboot.
```

## External Design

### `DmaRegionId`

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct DmaRegionId(pub u32);
```

The kernel's DMA table is keyed by `DmaRegionId`, not by frame
physical address.  `sys_dma_revoke` takes a `DmaRegionId`.

### `DmaRegionEntry` (post-RFC)

```rust
pub struct DmaRegionEntry {
    pub region_id:   DmaRegionId,
    pub owner_task:  TaskId,
    pub device_id:   DeviceId,           // NEW
    pub lease_id:    LeaseId,            // NEW
    pub user_va:     UserVa,             // unchanged, now actively used
    pub frame_pa:    PhysAddr,
    pub frame_count: u32,                // pages, not bytes
    pub state:       DmaRegionState,
    pub allocated_at:u64,                // tick
}

pub enum DmaRegionState {
    Active,
    Revoking,      // unmap in progress
    Quarantined,   // unmap failed; frame held but not freed
    Freed,         // unmap+zeroize succeeded; entry GC-able
}
```

### `sys_dma_alloc` (revised)

```text
1. Verify caller holds DmaAlloc cap with NET_DMA right (or similar).
2. Allocate `frame_count` contiguous frames.
3. Map them into caller's address space at a chosen user_va.
4. If any map_page() returns Err:
     - unmap all successfully mapped pages
     - sfence.vma
     - free the frames
     - return ResourceExhausted
5. On full success, allocate a DmaRegionId, insert DmaRegionEntry,
   audit DMA_REGION_CREATED, return (region_id, user_va).
```

Failure paths are total rollback — there is never a partial mapping.

### `sys_dma_revoke` (revised)

```text
1. Verify caller holds DmaAdmin cap (or owner).
2. Look up DmaRegionEntry by region_id.
3. Transition state to Revoking.
4. For each frame in user_va..user_va+frame_count*PAGE:
     - unmap PTE
     - if unmap returns Err, transition to Quarantined and break
5. sfence.vma to flush TLB.
6. If state == Revoking:
     - zeroize each frame
     - return each frame to allocator
     - transition to Freed
     - GC the entry
7. If state == Quarantined:
     - schedule quarantine timer (default: 60 s)
     - audit DMA_REGION_QUARANTINED
8. Audit DMA_REGION_REVOKED.
```

### Quarantine timer

A kernel-side timer entry; when it fires:

1. Retry unmap.
2. If still failing, leave Quarantined and log persistent error.
3. If retry succeeds, complete the revoke as in step 6 above.

The frame allocator MUST NOT hand out frames whose entries are in
`Quarantined` state.  This is enforced by the allocator consulting
the DMA table on `alloc_frame` candidates.

### Capability ABI

```rust
pub const NET_DMA:          CapRights = CapRights(1 << 16);
pub const DMA_REVOKE:       CapRights = CapRights(1 << 17);
pub const DMA_ADMIN:        CapRights = CapRights(1 << 18);
```

cap-broker policy assigns:

```text
- virtio-net driver: NET_DMA on its device's DmaAlloc capability,
  bound to a lease.
- cap-broker itself: DMA_ADMIN.
- All other services: nothing DMA-related.
```

## Data Model

### Audit events

```text
AUDIT_DMA_REGION_CREATED      = 0x0401
AUDIT_DMA_REGION_REVOKED      = 0x0402
AUDIT_DMA_REGION_QUARANTINED  = 0x0403
AUDIT_DMA_UNMAP_FAILED        = 0x0404
AUDIT_DMA_MAP_PARTIAL_ROLLBACK= 0x0405
```

All are pinned-critical.

### Semantic intents

```text
NET.DMA_REGION_REVOKED        = 0x01A0
NET.DMA_REGION_QUARANTINED    = 0x01A1
```

Visible in `proxy-text` as DMA lifecycle events.

## Internal Design

### Unmap implementation

```rust
// crates/fjell-kernel/src/mm/page_table.rs

/// Unmap a single 4 KiB page from the given task's address space.
/// Returns the old PA on success.
pub fn unmap_page(
    address_space: &mut AddressSpace,
    va: VirtAddr,
) -> Result<PhysAddr, MmError> { ... }

/// Issue sfence.vma flushing all entries.
pub fn sfence_vma_all() { unsafe { core::arch::asm!("sfence.vma") } }
```

These already exist in the v0.2 page-table module; this RFC wires
them into the DMA revoke path.

### Allocator coordination

```rust
impl FrameAllocator {
    fn alloc_frame(&mut self) -> Result<PhysFrame, MmError> {
        let candidate = self.next_free()?;
        if self.dma_table.is_quarantined(candidate.pa) {
            return Err(MmError::FrameQuarantined);
        }
        Ok(candidate)
    }
}
```

This adds a per-alloc dma-table lookup, but the table is small
(bounded by `DMA_TABLE_CAPACITY = 64`).

### Property tests

In `fjell-store-model` (or a new lightweight `fjell-dma-model`):

```text
D1: dma_revoke_unmaps_before_free
D2: revoked_va_returns_pagefault_on_access
D3: unmap_failure_quarantines_frame
D4: quarantined_frame_never_reused
D5: dma_alloc_partial_failure_total_rollback
D6: dma_revoke_idempotent_on_already_revoked
```

1000 cases per property.

### Acceptance tests per the crates review

```text
NEG:DMA:REVOKED_MAPPING_UNMAPPED:PASS
NEG:DMA:ZEROIZED_BEFORE_REUSE:PASS
NEG:DMA:UNMAP_FAILURE_QUARANTINES_FRAME:PASS
NEG:DMA:REVOKE_UNMAPS_USER_VA:PASS
```

These are QEMU smoke markers, set by a dedicated negative-test
service (`fjell-neg-test` v0.7.4 extension).

## Security Design

### Threat model

| Threat | Pre-RFC | Post-RFC |
|--------|---------|----------|
| Stale VA after revoke | Live, points at reused frame | Unmapped; faults on access |
| Frame reused before zeroize | Possible | Impossible: zeroize precedes return-to-allocator |
| Unmap failure leaves frame mapped to attacker | Possible | Frame quarantined; allocator skips it |
| Map partial failure leaves half-mapped region | Possible | Total rollback; no live region exists |
| Cap-less DMA alloc | Currently broad caps | Narrowed by RFC-v0.7.4-003 |

### Audit posture

Every DMA lifecycle transition emits a pinned-critical audit event.
A v0.7.4 device that misbehaves cannot escape audit trace.

### Recovery

Quarantined frames are kept out of the allocator pool indefinitely.
A future "drain quarantine" admin operation may re-unmap and
recover them; v0.7.4 does not implement this (quarantined frames are
simply leaked until reboot — acceptable for a 64-entry DMA table).

## Memory / Resource Design

- `DmaTable`: 64 entries × ~64 B = ~4 KiB kernel.
- `DmaRegionId` allocation: u32, never reused (drift-free).
- Per-region tracking: ~64 B in kernel + user VA range in caller.

## Compatibility and Migration

- `sys_dma_alloc` signature changes: returns `(DmaRegionId, UserVa)`
  instead of `(PhysAddr, UserVa)`.  The PA is no longer exposed to
  the caller.
- `sys_dma_revoke` takes `DmaRegionId` instead of `PhysAddr`.
- Existing v0.4 networking code is updated (RFC-v0.7.3-001 takes a
  hard dependency).
- Audit event IDs are additive; no replacement.

## Test Strategy

```text
- 6 property tests × 1000 cases.
- 4 NEG QEMU smoke markers per the crates review.
- Unit test: allocator skips quarantined frame.
- Unit test: partial-rollback never leaves a half-mapped region.
- Integration: virtio-net allocates, uses, revokes, and the original
  user_va faults on access post-revoke.
```

## Acceptance Criteria

```text
- All 4 NEG:DMA:* markers green.
- 6 DMA property tests green.
- No release path returns a frame to the allocator without zeroize.
- No release path frees a region without unmapping user_va first.
- Quarantine timer is wired to the existing kernel scheduler.
- ADR-v0.7.4-001 filed.
- RFC-v0.7.3-001 (net runtime) no longer blocked.
```

## Documentation Requirements

```text
- docs/src/internals/dma-lifecycle.md — full state machine and
  invariants.
- docs/src/reference/dma-syscalls.md updated for new signatures.
- UNSAFE_CHARTER.md — DMA category invariants documented.
- ADR-v0.7.4-001 — CRITICAL fix and rationale.
```

## Open Questions

```text
1. Quarantine retry policy: how many retries before giving up?
   Proposal: 3 retries on a 60/120/240-second schedule, then
   permanent quarantine until reboot.

2. Should we expose a sys_dma_quarantine_status() syscall for
   observability? Proposal: no — semantic-stream intents are
   sufficient.

3. Frame-count limits per region — should we cap? Proposal: yes,
   16 pages = 64 KiB per region. virtio-net descriptor rings need
   ~4 KiB; this leaves headroom.
```

## Release Gate

```text
- All 4 NEG:DMA:* markers in QEMU smoke
- 6 property tests in CI
- ADR-v0.7.4-001 accepted
- Frame allocator updated to skip quarantined frames
```

**This is the highest-priority RFC of the v0.7.x series.**
RFC-v0.7.3-001 (networking) is blocked until this is accepted.
