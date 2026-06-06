# RFC 036: DmaRegion capability, zeroize, and quarantine

**RFC ID:** 036  
**Also known as:** RFC-v0.2-006  
**Status:** Implemented (v0.2.0)
**Target version:** v0.2.0  
**Phase:** Phase 4 — DMA Boundary Closure  
**Related epics:** C (MMIO/DMA), A (Unified Capability Enforcement)

## Problem

At v0.1.0 the DMA path is the most under-protected boundary in the
system.  RFC 007 introduced a per-task DMA allocator, but ownership,
revoke semantics, and cleanup are still informal:

- A revoked DMA region may still hold device-visible bytes.
- A misbehaving device may prevent cleanup indefinitely.
- There is no formal "this page is unsafe to reuse" state.

RFC 030 lists this as a v0.2 closure target.

## Proposed fix

### DmaRegion object

```rust
pub struct DmaRegion {
    pub id:          DmaRegionId,
    pub owner:       TaskId,
    pub device:      DeviceId,
    pub lease_id:    LeaseId,
    pub user_va:     VirtAddr,
    pub frames:      FixedVec<PhysFrame, MAX_DMA_FRAMES>,
    pub state:       DmaRegionState,
    pub revoke_tick: Option<u64>,
}

pub enum DmaRegionState {
    Active,
    Revoked,
    Quarantined,
    Zeroized,
    Freed,
}
```

### v0.2 restriction: 1-page maximum

`MAX_DMA_FRAMES = 1`.  A single DmaRegion holds at most one
physical page.  Multi-page scatter-gather is deferred until a
device that needs it lands (no current driver does).

### Capability

`CapKind::DmaRegion` with `DMA_ALLOC`, `DMA_USE`, `DMA_REVOKE`
rights from RFC 031.

### Revoke flow

```
1. mark region Revoked
2. prevent future use (require_cap rejects)
3. unmap user mapping
4. zeroize page
5. quarantine or free frame
6. emit audit event
```

### Quarantine for uncertain device quiesce

If the device cannot be guaranteed to have drained:

```
- enter Quarantined
- devmgr receives a timeout budget (revoke_tick + budget)
- timeout triggers device reset path
- page is zeroized after reset or forced quarantine completion
- frame is not reused until zeroized
```

A misbehaving device must not block cleanup forever — the timer
fail-safe (RFC 037) bounds the quarantine duration.

### Lifecycle revoke on task exit

When a task exits or faults, every DmaRegion it owns is revoked and
walked through the same zeroize/quarantine pipeline.  This is the
mechanism that guarantees no DMA buffer outlives the task that
created it.

## Rationale

The four-state cleanup machine
(`Active → Revoked → Quarantined → Zeroized → Freed`) makes the
"is this page safe to reuse?" question a single state check.
Combining the states into a flag-tuple was rejected as harder to
audit.

The 1-page restriction is intentional: physical contiguity for
multi-page DMA requires either a buddy allocator or scatter-gather
metadata, both of which are larger designs than v0.2 should
attempt.  The current driver (virtio-blk) operates one descriptor
per page, so this restriction does not block anything shipping.

The quarantine timeout is what prevents a hostile or stuck device
from blocking system cleanup forever.  Without it, a single
misbehaving driver could leak all DMA pages.

## Impact

- Crates: `fjell-kernel` (DMA module, allocator, zeroize), `fjell-cap`
  (new CapKind), `fjell-devmgr` (region grant + reset path),
  `fjell-driver-virtio-blk` (call site update), `fjell-abi`
  (new syscall numbers, new errors).
- Backward compatibility: **breaks** the v0.1.x DMA ABI.
- Audit: every state transition emits a record.

## Test plan

### QEMU negative tests
- `NEG:DMA:ALLOC_WITHOUT_CAP:PASS`
- `NEG:DMA:SIZE_TOO_LARGE:PASS`
- `NEG:DMA:REVOKED_REGION_REJECTED:PASS`
- `NEG:DMA:ZEROIZED_ON_EXIT:PASS`
- `NEG:DMA:QUARANTINE_TIMEOUT:PASS`
- `NEG:DMA:QUARANTINED_PAGE_NOT_REUSED:PASS`

### Acceptance gates
- DMA memory cannot be reused before zeroize/quarantine completion.
- DMA cleanup does not wait forever on device cooperation.
- DMA ownership is explicit (owner + device + lease).
- DMA revoke emits an audit event.

## Implementation notes

- Out of scope: IOMMU (v0.3), multi-page scatter-gather, contiguous
  multi-page allocation, multiple-device DMA arbitration.
- Zeroize must use a fence appropriate to the architecture: on
  RISC-V, `fence rw,rw` between the memset and frame return.
- The quarantine timeout default budget should be conservative
  (≥ 100 ms in tick units) so that legitimate device drain is not
  starved.  Specific values are tuned in implementation.
- Invariants:
  - `DMA-001` No DMA reuse before zeroize.
  - `DMA-002` Quarantine has a bounded duration.
  - `DMA-003` Revoke does not block on device cooperation.
  - `DMA-004` Owner task exit triggers full DMA cleanup.
