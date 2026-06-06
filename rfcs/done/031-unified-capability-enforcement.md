# RFC 031: Unified capability enforcement

**RFC ID:** 031  
**Also known as:** RFC-v0.2-001  
**Status:** Implemented (v0.2.0)
**Target version:** v0.2.0  
**Phase:** Phase 1 — Capability Enforcement Core  
**Related epics:** A (Unified Capability Enforcement), B (Lease Revocation), C (MMIO/DMA), F (cap-broker)

## Problem

At v0.1.0, authority checking is scattered across kernel syscalls.
Different syscalls use different helpers: `caller_has_cap(kind)`,
task-id allowlists, debug-only bypasses, and implicit bootstrap
authority.  This produces:

- Inconsistent enforcement: a missing right may be checked at one
  call site and silently accepted at another.
- An untestable security boundary: there is no single function whose
  correctness covers “did this call have authority”.
- Lease state is not consulted by most call sites.

The v0.1.x capability/lease enforcement audit (RFC 029) makes the
gaps explicit; this RFC closes them.

## Proposed fix

Introduce one unified authority-checking path used by every kernel
operation that requires authority:

```rust
pub fn require_cap(
    task: TaskId,
    handle: CapHandle,
    expected_kind: CapKind,
    required_rights: CapRights,
    required_scope: Option<ObjectScope>,
) -> Result<CapabilityRef, CapError>;
```

Check order is normative:

```
1. CSpace lookup
2. handle generation check
3. cap state check
4. kind check
5. rights check
6. scope check
7. lease check
```

### Affected syscall families

Capability: `cap_copy`, `cap_mint`, `cap_revoke`, `cap_inspect`.
IPC: `ipc_send`, `ipc_recv`, `ipc_try_recv`, `ipc_call`, `ipc_reply`.
Task: `task_spawn`, `task_start`, `task_status`, `task_kill` (if
present).
Lease: `lease_create`, `lease_revoke`, `lease_inspect`.
Device: `mmio_map`, `dma_alloc`, `dma_revoke`.
Audit: `audit_drain`.
Boot/System: `boot_evidence_get`, `reboot`.

### Required types

```rust
bitflags::bitflags! {
    pub struct CapRights: u64 {
        const READ        = 1 << 0;
        const WRITE       = 1 << 1;
        const EXECUTE     = 1 << 2;
        const SEND        = 1 << 3;
        const RECV        = 1 << 4;
        const CALL        = 1 << 5;
        const REPLY       = 1 << 6;
        const COPY        = 1 << 7;
        const MINT        = 1 << 8;
        const REVOKE      = 1 << 9;
        const INSPECT     = 1 << 10;
        const DROP        = 1 << 11;
        const TASK_CREATE = 1 << 12;
        const TASK_START  = 1 << 13;
        const TASK_STATUS = 1 << 14;
        const TASK_KILL   = 1 << 15;
        const LEASE_CREATE  = 1 << 16;
        const LEASE_REVOKE  = 1 << 17;
        const LEASE_INSPECT = 1 << 18;
        const MMIO_MAP    = 1 << 19;
        const DMA_ALLOC   = 1 << 20;
        const DMA_USE     = 1 << 21;
        const DMA_REVOKE  = 1 << 22;
        const AUDIT_DRAIN = 1 << 23;
        const BOOT_READ   = 1 << 24;
        const REBOOT      = 1 << 25;
    }
}

pub enum CapKind {
    Endpoint, Reply, TaskControl, TaskCreate, TaskInspect,
    LeaseAdmin, MmioRegion, DmaRegion, AuditDrain,
    BootEvidence, Reboot, PersistentStore, BootControl,
    UpgradeTransaction, Verification, RootfsRead,
    SnapshotCreate, SnapshotRead,
}

pub enum ObjectScope {
    Any,
    Object(ObjectId),
    Task(TaskId),
    Endpoint(EndpointId),
    Lease(LeaseId),
    MmioRegion(MmioRegionId),
    DmaRegion(DmaRegionId),
    StoreNamespace(StoreNamespaceId),
    BootSlot(SlotId),
}

pub struct Capability {
    pub generation: u16,
    pub state:      CapState,
    pub kind:       CapKind,
    pub object_id:  ObjectId,
    pub rights:     CapRights,
    pub scope:      ObjectScope,
    pub parent:     Option<CapHandle>,
    pub lease:      Option<LeaseBinding>,
    pub flags:      CapFlags,
}

pub enum CapState { Empty, Active, Dropped, Revoked }

pub enum CapError {
    InvalidHandle, GenerationMismatch, EmptySlot,
    Dropped, Revoked, WrongKind, MissingRight,
    ScopeMismatch, LeaseRevoked, LeaseExpired,
    LeaseGenerationMismatch, Internal,
}
```

### Rights amplification rule

`cap_mint` must enforce both:

```
child_rights ⊆ parent_rights
child_scope ⊆ parent_scope
```

No child capability may gain broader rights, broader object scope, or
a removed lease binding (except along the explicit kernel bootstrap
path).

### Bootstrap exception

Bootstrap capabilities may have `lease = None`, `scope = Any`, but
only the initial `init` task may receive these.  All non-bootstrap
service grants must be lease-bound.

### Syscall-visible error mapping

```
InvalidHandle / GenerationMismatch → InvalidCap
WrongKind                          → WrongKind
MissingRight / ScopeMismatch       → PermissionDenied
LeaseRevoked / LeaseExpired        → LeaseRevoked
```

## Rationale

A single function with a fixed check order is the only way to make
authority checking provable and testable.  Type-only checks
(`caller_has_cap(kind)`) cannot enforce per-object scope or lease
state; they are silently insufficient and must be removed.

Fixing the order matters: stale handles must fail before any
object-specific logic runs.  Wrong-kind must fail before
rights-bitfield interpretation.  Lease check is last because it is
the most expensive (table lookup); checking cheap fields first
preserves O(1) hot-path cost.

`cap_drop` is intentionally exempt from the lease check (see RFC 033
§2.8): a task must always be able to release a dead slot.

## Impact

- Crates: `fjell-cap`, `fjell-kernel` (syscall entry points),
  `fjell-abi` (error codes), `fjell-syscall` (user-side mapping),
  `fjell-service-api` (return-type breaks).
- Backward compatibility: **breaks** the v0.1.x syscall ABI for any
  service that depended on debug-bypass or type-only authority.
  v0.2 is a breaking-change release; this is expected.
- Audit: every authority-check failure emits an audit event
  (`AuditKind::CapabilityCheckFailed`).

## Test plan

### Unit tests
- `require_cap` accepts a valid cap.
- `require_cap` rejects stale generation, wrong kind, missing right,
  scope mismatch, revoked lease.
- `cap_mint` rejects rights amplification and scope amplification.

### QEMU negative tests
- `NEG:CAP:MISSING_RIGHT:PASS`
- `NEG:CAP:WRONG_KIND:PASS`
- `NEG:CAP:GENERATION_MISMATCH:PASS`
- `NEG:CAP:SCOPE_MISMATCH:PASS`
- `NEG:CAP:REVOKED_LEASE:PASS`

### Acceptance gates
- `require_cap()` is the only production authority-checking helper.
- `caller_has_cap()` is removed or debug-only.
- All authority-bearing syscalls call `require_cap()`.
- A grep for `caller_has_cap` in production paths returns zero hits.

## Implementation notes

- Out of scope: recursive revocation in the kernel, cap-broker
  policy evaluation (RFC 044), full MMIO/DMA object model (RFCs 039,
  040), service separation (RFCs 041, 042), networking, v1 ABI
  stabilisation.
- Documentation updates required:
  `docs/architecture/capability-enforcement.md`,
  `docs/security/capability-threat-model.md`,
  `docs/verification/capability-invariants.md`,
  `docs/abi/syscall-capability-requirements.md`.
- The initial `ObjectScope` implementation may support only `Any`,
  `Task`, `Endpoint`, `Lease`, `MmioRegion`, `DmaRegion`.  Other
  variants are stubs whose checks always succeed; the API shape must
  accept them so later RFCs can wire them in without an ABI change.
