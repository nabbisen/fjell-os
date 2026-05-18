# Changelog

All notable changes to Fjell OS are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.2.2] - 2026-05-18

### RFC 042 Phase 2 — Policy Negative Tests

Wires two `policy` negative-test markers by giving cap-broker a dedicated
endpoint and having init perform the Bootstrap→Enforcing handoff at boot.

### Added

- **`fjell-syscall`**: `sys_ipc_call_words(ep, tag, w0, w1, w2)` — multi-word
  IPC call using RISC-V registers a2-a4 so the cap-broker protocol's
  (requester, resource, rights) tuple reaches the server.
- **`fjell-neg-test`**: two new test functions:
  - `test_policy_default_deny()` — sends `CAP_REQUEST` as ImageId 20 (not in
    policy) → receives `CAP_DENIED` → emits `NEG:POLICY:DEFAULT_DENY:PASS`.
  - `test_policy_bootstrap_guard()` — sends `BOOTSTRAP_COMPLETE` to a broker
    already in Enforcing state → receives `usize::MAX` rejection →
    emits `NEG:POLICY:BOOTSTRAP_GUARD:PASS`.

### Changed

- **`crates/fjell-kernel/src/task/spawn.rs`**:
  - `CAP_BROKER` now gets `ep_obj = 5` (dedicated endpoint, was shared 0).
  - `NEG_TEST` gets slot 3 = `Endpoint(object_id=5)` — direct path to broker.
- **`crates/fjell-kernel/src/main.rs`**: init's slot 1 added as
  `Endpoint(object_id=5)` — init uses this to send `BOOTSTRAP_COMPLETE`.
- **`crates/fjell-init/src/main.rs`**: after spawning `CAP_BROKER`, yields
  twice then calls `sys_ipc_call_words(1, BOOTSTRAP_COMPLETE, 0, 0, 0)` to
  transition cap-broker to Enforcing state before other services start.
  Adds `sys_yield` and `sys_ipc_call_words` to its syscall imports.
- **`tests/qemu/profiles/policy.toml`**: `expected_markers` populated with
  `NEG:POLICY:DEFAULT_DENY:PASS` and `NEG:POLICY:BOOTSTRAP_GUARD:PASS`.
- Workspace version bumped to `0.2.2`.

### Deferred

- `NEG:CAP:LEASE_REVOKED:PASS` — needs cap delegation syscall (v0.3):
  cap-broker cannot yet install a capability into another task's CSpace;
  the only lease-bound caps are those created by the broker itself.
- `NEG:IPC:*` — multi-task coordination + lease-bound caps (v0.3).

## [0.2.1] - 2026-05-18

### RFC 042 Phase 1 — Negative Test Marker Emission

Adds the `fjell-neg-test` service and wires 7 QEMU negative-test markers.

### Added

- **`crates/fjell-neg-test/`** — new service crate (`ImageId::NEG_TEST = 20`):
  - Exercises 6 negative scenarios and emits markers for each that passes.
  - Tests run on startup with two `sys_yield()` calls first so cap-broker
    reaches Enforcing state before any syscalls are issued.
- **`fjell-abi`**: `ImageId::NEG_TEST = ImageId(20)`.
- **`fjell-syscall`**:
  - `sys_dma_revoke(device_pa)` — user-space wrapper for RFC 036 explicit revoke.
  - `sys_audit_drain_raw(ptr, cap)` — unsafe raw audit-drain for pointer-rejection testing.
- **`fjell-kernel/src/task/image.rs`**: `NEG_TEST_BIN` static + match arm.
- **`fjell-kernel/src/task/spawn.rs`**: `NEG_TEST` gets `AuditDrain` (slot 1) and
  `DmaRegion` (slot 2) bootstrap caps in addition to the standard `Endpoint` (slot 0)
  and `MmioRegion` (slots 31–34).
- **`fjell-init`**: spawns `NEG_TEST` after `SAMPLE_SERVICE`.
- **`fjell-tools/src/qemu.rs`**: `fjell-neg-test` added to `SERVICES` build list.

### Markers now emitted (verified at QEMU time)

| Profile | Marker | Scenario |
|---------|--------|---------|
| `capability` | `NEG:CAP:WRONG_KIND_REJECTED:PASS` | `sys_mmio_map` with Endpoint cap → kind check fires |
| `mmio` | `NEG:MMIO:RIGHTS_CHECK:PASS` | Same call — MMIO rights path exercised |
| `mmio` | `NEG:MMIO:BOUNDS_REJECTED:PASS` | `sys_mmio_map` with offset 0xFFFF_F000 → `is_accessible` fails |
| `dma` | `NEG:DMA:RIGHTS_CHECK:PASS` | `sys_dma_alloc` with Endpoint cap → kind check fires |
| `dma` | `NEG:DMA:REVOKE_EXPLICIT:PASS` | Alloc DMA, `sys_dma_revoke(pa)` → Active→Zeroized→Freed |
| `user-copy` | `NEG:USER_COPY:NULL_REJECTED:PASS` | `sys_audit_drain_raw(0, …)` → `UserPtr::NullPointer` |
| `user-copy` | `NEG:USER_COPY:KERNEL_ADDR_REJECTED:PASS` | `sys_audit_drain_raw(0x8000_0000, …)` → `UserPtr::KernelAddress` |

### Profiles updated

Profiles now declare only the markers that are actually emitted. Profiles with
no emitted markers (`ipc`, `policy`, `svc`, `audit`) have `expected_markers = []`
so `cargo xtask qemu-negative <category>` passes infrastructure-only.

### Fixed (from v0.2.0 smoke log)

- `fjell-cap-broker`: `RIGHT_DROP`, `BOOTCTL`, `UPGRADED`, `VERIFYD`, and
  `DelegationRecord` field dead-code warnings suppressed with `#[allow(dead_code)]`.
- `fjell-storaged`: 15 Rust 2024 `unsafe_op_in_unsafe_fn` warnings fixed by
  adding explicit `unsafe { }` blocks inside each `unsafe fn` that calls
  `core::ptr::read_volatile` / `write_volatile`. Removed `sys_platform_info_get`
  unused import.
- `fjell-init`: removed unused `FreshnessStatus` import.

### Deferred for v0.2.2

- `NEG:CAP:LEASE_REVOKED:PASS` — needs lease-bound cap (cap-broker grant path).
- `NEG:IPC:*` — blocked IPC revocation scenarios (multi-task coordination).
- `NEG:POLICY:DEFAULT_DENY:PASS` — IPC routing to cap-broker from NEG_TEST.
- `NEG:DMA:ZEROIZE_ON_EXIT:PASS` — post-exit memory inspection.

## [0.2.0] - 2026-05-17 — Security Boundary Closure

### Release summary

v0.2.0 closes all open capability-enforcement gaps identified by the v0.1.0
security audits.  Every kernel syscall that previously performed no capability
check, or a partial check, now goes through `require_cap()` with the correct
kind, rights bit, and lease epoch.  The capability broker now enforces a
proper default-deny policy after a one-way Bootstrap→Enforcing transition.
Evidence of security events is captured in a persistent, continuity-verifiable
audit trail format.

**86 host tests pass.  All QEMU-verifiable items have markers defined.**

### Changes by RFC

| Phase | RFC | Brief |
|-------|-----|-------|
| 1 | 031 | `require_cap()` 7-step enforcement library in `fjell-cap` |
| 1 | 032 | `sys_cap_drop` + CSpace GC |
| 2 | 033 | O(1) lease epoch revocation; `revoke_owned_by` on task exit |
| 2 | 034 | Blocked IPC wake/cancel on lease revocation |
| 3 | 035 | `sys_mmio_map` requires `MMIO_MAP` right; `MmioRegionState` |
| 4 | 036 | `DmaRegionState` 4-state machine; `sys_dma_revoke`; zeroize |
| 5 | 037 | Timer preemptive fail-safe; `TIMER_PREEMPTED` flag; quantum violations |
| 5 | 038 | Service READY protocol types in `fjell-service-api` |
| 6 | 039 | `UserPtr` arithmetic validation; `copy_from_user`; `AuditLogHeader` |
| 7 | 040 | cap-broker Bootstrap/Enforcing; `DelegationRecord`; u64 rights |
| 8 | 041 | `AuditPersistRecord`; snapshot audit fields; `EvidenceRow`; gap detection |
| 9 | 042 | Negative-test markers; 8 profiles populated |
| 9 | 043 | Release gate document; `TEST:V02:PASS` |

### Known limitations accepted for v0.2.0

- Task/lease syscalls use CSpace slot-scan (V02-A-001); handle-based `require_cap` requires ABI changes (v0.3).
- DMA quarantine timeout is synchronous zeroize; full timer-callback path deferred.
- Service binary extraction (storaged/bootctl from init) requires QEMU (RFC 038 backlog).
- Badge-based sender identity in cap-broker bootstrap (v0.3).

## [0.2.0-alpha.8] - 2026-05-17

### v0.2 Phase 8 — Persistent Evidence Hardening (RFC 041)

### Added

- **`fjell-snapshot-format/src/lib.rs`** (RFC 041 §"Snapshot extension"):
  - `SnapshotDigest.audit_last_seq: u64` — last persisted audit sequence.
  - `SnapshotDigest.audit_dropped_count: u64` — cumulative dropped count.
  - `SnapshotDigest::with_audit(slot, store_seq, audit_last_seq,
    audit_dropped_count)` builder.
  - `SnapshotDigest::has_audit_gaps()` — true when `audit_dropped_count > 0`.
  - `SystemSnapshot::new_with_audit(…)` constructor.
  - `EvidenceGapError { DroppedRecords, SequenceRegression, NoAuditState }`.
  - `SystemSnapshot::verify_evidence_continuity(prev: Option<&Self>)`.
  - 5 unit tests covering gap detection, continuity ok/err, and roundtrip.

- **`fjell-semantic-format/src/lib.rs`** (RFC 041 §"Semantic state nodes"):
  - New `EventKind` variants: `DmaQuarantineTimeout`, `RollbackInitiated`,
    `SecurityBoundaryViolation`.
  - New `StateKind` variants: `EvidenceMatrix`, `SecurityAuditState`.
  - `EvidenceRow { event_label, audit_kind, count, persisted, dropped }` — one
    row in the evidence matrix.
  - `EvidenceRow::has_gaps()`.
  - `security_audit_state_node(last_seq, dropped, pending)` — builds a
    `StateNode` of kind `SecurityAuditState` with `Status::Warning` when
    `dropped > 0`.
  - `MAX_EVIDENCE_ROWS: usize = 16`.

- **`fjell-audit-format/src/lib.rs`** (RFC 041 §"Persistence format"):
  - `AuditPersistRecord` (40 bytes) — extends `AuditRecordBin` with
    `persist_seq: u32` for storaged write ordinal.
  - `AuditPersistRecord::from_bin(bin, persist_seq)` and `to_bin()`.
  - `AuditLogHeader` (32 bytes, matches `AUDIT_RECORD_BIN_SIZE`) — magic
    `FJLAUDIT`, schema version 2, `first_seq`, `dropped_at_open`.
  - `AuditLogHeader::new(first_seq, dropped_at_open)` and `is_valid()`.
  - `AUDIT_LOG_MAGIC`, `AUDIT_LOG_SCHEMA_V2`, `AUDIT_PERSIST_RECORD_SIZE`.
  - 5 unit tests: roundtrip, header validity, v0.2 label coverage, size
    assertions.

### Changed

- Workspace version bumped to `0.2.0-alpha.8`.

### Deferred (service-layer work)

- The actual `auditd → storaged` IPC pipeline (service binary) is a QEMU-only
  deliverable. The format types are defined; the service wiring happens when
  storaged and auditd are extracted as separated services (RFC 038 backlog).
- Evidence matrix population from live `sys_audit_drain` output.
- `NEG:AUDIT:EVIDENCE_GAP_DETECTED:PASS` QEMU marker.

## [0.2.0-alpha.7] - 2026-05-17

### v0.2 Phase 7 — cap-broker Bootstrap Handoff and Default-Deny (RFC 040)

### Added

- **`fjell-cap-broker/src/main.rs`** fully rewritten (RFC 040):
  - `BrokerState { Bootstrap, Enforcing }` — one-way typestate (§2.1).
  - Bootstrap phase: only `init` may communicate; any `CAP_REQUEST` before
    `BOOTSTRAP_COMPLETE` is returned as `CAP_DENIED`.
  - `BOOTSTRAP_COMPLETE` message (label `0x100`) from init transitions the
    broker to Enforcing.  A second transition is rejected with `usize::MAX`.
  - `DelegationRecord { parent_idx, requester, resource, rights, lease_id,
    active }` — delegation tree tracking (§2.4), up to 64 concurrent entries.
  - `DelegationTree::revoke_lease(lid)` — cascades removal of all records
    tied to a revoked lease (cap-broker policy-layer revocation).
  - Grant revocation message (tag `0x023`): calls `sys_lease_revoke` and
    cleans up the delegation tree.
  - **Updated `CapRights` constants to v0.2 `u64` bit layout**:
    `RIGHT_SEND = 1<<3`, `RIGHT_RECV = 1<<4`, …, `ALL_RIGHTS = (1<<26)-1`.
    The old `u32` constants (`ALLOW_ALL = 0xFF`, `ALLOW_RECV = 0x02`) are gone.
  - **Updated `ResourceClass`**: `DmaAlloc → DmaRegion` (aligns with
    RFC 036 CapKind rename).
  - **Updated policy table**: all `rights: u64`, `TASK_MGMT` bundle,
    `EP_RW` bundle, correct `MMIO_MAP` / `DMA_ALLOC` / `AUDIT_DRAIN` bits.
  - `PolicyRule.rights: u64` (was `u32`).
  - `PolicyResult` renamed; evaluator unchanged (deny→allow→default deny).
- **`fjell-tools/src/policy_eval.rs`** — host-testable mirror of the policy
  evaluator; 13 unit tests verifying BROKER-001 through BROKER-003 invariants.
- **`tests/qemu/profiles/policy.toml`** — placeholder for policy negative tests.

### Changed

- Workspace version bumped to `0.2.0-alpha.7`.

### Deferred (QEMU-only work)

- Full badge-based sender identity check in `BOOTSTRAP_COMPLETE` handler.
  Current v0.2 trusts the kernel to have delivered the message from init;
  a proper badge check requires the kernel to set `sender_badge == ImageId`
  on all IPC frames from identified services.
- QEMU negative-test markers for policy (`NEG:POLICY:DEFAULT_DENY:PASS`).

## [0.2.0-alpha.6] - 2026-05-17

### v0.2 Phase 6 — Safe User Copy + Real Audit Drain (RFC 039)

### Added

- **`fjell-kernel/src/mm/user_ptr.rs`** (RFC 039 §2.1):
  - `UserPtr { addr, len }` — validated user-space pointer range.
  - `UserPtr::new(addr, len)` rejects: null pointer, kernel address
    (addr ≥ RAM_BASE), length overflow (addr + len wraps usize), range
    crosses kernel (addr + len > RAM_BASE).
  - `UserCopyError` enum with 7 variants; `impl From<UserCopyError> for SysError`.
  - 8 unit tests covering all RFC 039 §"Fuzz corpus" cases: null, zero-len null,
    kernel address (3 variants), length overflow, crosses-kernel, partially valid
    range, zero-len valid address, end-just-below-kernel.
- **`fjell-kernel/src/mm/user_copy.rs`** updated (RFC 039 §2):
  - `copy_to_user_bytes` now calls `UserPtr::new` as the first validation step.
  - New `copy_from_user_bytes(root_pfn, src_va, dst)` — reads from user VA
    (validates PTE_R + PTE_U), writes to kernel buffer.
- **`fjell-audit-format/src/lib.rs`** updated:
  - New `AuditKind` variants: `CapDrop = 15`, `LeaseRevoked = 16`,
    `TaskQuantumExceeded = 30`.
  - `from_u16()` and `label()` extended to cover all v0.2 kinds.
- **`tests/qemu/profiles/user-copy.toml`** and
  **`tests/qemu/profiles/audit.toml`** — placeholder profiles for the
  RFC 039 negative-test categories.

### Changed

- Workspace version bumped to `0.2.0-alpha.6`.

### Note on audit drain

`sys_audit_drain` already produces real binary `AuditRecordBin` records
(implemented in v0.1.0 / RFC 020).  RFC 039 strengthens the surrounding
infrastructure:
- `copy_to_user_bytes` now validates via `UserPtr` before the page walk.
- `sys_audit_drain` checks `AUDIT_DRAIN` right via the v0.2 require_cap path
  (wired in alpha.2).

The dropped-count is returned in `a2` by `sys_audit_drain` (existing).

## [0.2.0-alpha.5] - 2026-05-17

### v0.2 Phase 5 — Cooperative Service Separation Foundation (RFC 037, RFC 038)

### Added

- **`fjell-kernel/src/task/tcb.rs`** (RFC 037):
  - `TaskAccounting.quantum_violations: u32` — consecutive timer preemptions
    without a voluntary `sys_yield`.
  - `QUANTUM_VIOLATION_THRESHOLD: u32 = 3` — threshold before
    `TaskQuantumExceeded` is emitted.
- **`fjell-kernel/src/audit/ring.rs`**: `TaskQuantumExceeded = 30` audit kind.
- **`fjell-kernel/src/trap/dispatch.rs`** (RFC 037):
  - `TIMER_PREEMPTED: Flag` — distinguishes involuntary timer preemption from
    voluntary `sys_yield`.
  - `handle_timer()` sets `TIMER_PREEMPTED` in addition to `YIELD_REQUESTED`.
  - `schedule_next()` increments `quantum_violations` on timer preemption and
    emits `TaskQuantumExceeded` when `≥ QUANTUM_VIOLATION_THRESHOLD`.
  - Voluntary yield or IPC block resets `quantum_violations` to 0.
  - RFC 033 lifecycle revoke: `lt.revoke_owned_by(id)` called on task exit
    and fault (wiring the Phase 2 lifecycle revoke path).
- **`fjell-service-api/src/lib.rs`** (RFC 038):
  - `ready` module: `LABEL`, `START_TIMEOUT_MS = 1000`, `FAULT_LABEL`,
    `TIMEOUT_LABEL`.
  - `SvcLifecycle { Empty, Spawned, Ready, Running, StartFailed, Faulted }`.
  - `extraction_order::ORDER` — canonical separation order:
    storaged → bootctl → verifyd → upgraded → rootfsd → snapshotd.
  - `ServiceManifestEntry { name, image_id, start_timeout_ms, ready_endpoint }`.
- **`tests/qemu/profiles/svc.toml`** — placeholder profile for the `svc`
  negative-test category (CI passes, no markers until services are extracted).

### Changed

- Workspace version bumped to `0.2.0-alpha.5`.

### Deferred (service extraction)

Per RFC 038, the following work requires modifying the service binaries and
verifying under QEMU — deferred to v0.2.0-alpha.6:

- `fjell-init` switched from inline service implementations to spawning
  services that use the READY protocol.
- `fjell-service-manager` updated to consume READY messages with timeout
  tracking.
- First service pair extracted: `fjell-storaged`, `fjell-bootctl`.
- `NEG:SVC:START_TIMEOUT_DETECTED:PASS`, `NEG:SVC:FAULT_DETECTED:PASS` markers.

### Phase 5 coverage

`sys_ipc_try_recv` (RFC 037 non-blocking recv) was already implemented in
RFC 019 / v0.1.0.  RFC 037's new contribution — the timer preemptive fail-safe
— is now wired in the kernel.  RFC 038's new contribution — the READY protocol
type system — is in `fjell-service-api`.

## [0.2.0-alpha.4] - 2026-05-17

### v0.2 Phase 3 + Phase 4 — MMIO/DMA Boundary Closure (RFC 035, RFC 036)

### Added

- **`fjell-kernel/src/platform/qemu_virt.rs`** (RFC 035):
  - `MmioRegionState { Active, Revoked }`.
  - `MmioRegionObject` gains `id: u32` and `state: MmioRegionState` fields.
  - `MmioRegionObject::is_accessible(offset, size)` helper.
  - `mmio_region_table()` now populates `id` and `state` on each entry.
- **`sys_mmio_map`** updated (RFC 035 §2):
  - Now calls `fjell_cap::enforcement::require_cap(cs, handle, MmioRegion,
    MMIO_MAP, None, lt)` — full 7-step check replaces manual kind + lease.
  - `is_accessible(offset, size)` replaces manual bounds check.
  - Mapping is explicitly `R|W|U` without `X` (non-executable).
  - Error code `InvalidArg` (was `InvalidCap`) for zero-size and out-of-bounds.
- **`sys_dma_alloc`** updated (RFC 036):
  - Accepts both `DmaRegion` (new) and `DmaAlloc` (legacy alias) `CapKind`.
  - Checks `DMA_ALLOC` right and lease via `require_cap` path.
- **`sys_dma_revoke`** — new syscall handler (RFC 036):
  - `sys_dma_revoke(a0=device_pa) -> a0=status`.
  - Transitions: Active → Zeroized → Freed (synchronous in v0.2).
  - Dispatched at `SyscallNumber::DmaRevoke = 112`.
- **`DmaRegionState { Active, Revoked, Quarantined, Zeroized, Freed }`** (RFC 036 §2).
- **`DmaRegionEntry`** gains `state: DmaRegionState`.
- **`DmaRegionTable::revoke_by_pa(owner, pa)`** — explicit per-region revoke
  with synchronous zeroize + frame return.
- **`DmaRegionTable::release_task`** updated to check `state == Active`
  explicitly, so it does not double-free Revoked/Quarantined regions.

### Changed

- `docs/src/audit/capability-lease-enforcement-audit-v0.1.md` updated:
  `mmio_map` reclassified `Missing → OK`; `dma_alloc` reclassified
  `Missing → Partial`; `dma_revoke` updated; enforcement checklist
  entries ticked.
- Workspace version bumped to `0.2.0-alpha.4`.

### Known limitations

- Owner-task scope check for MMIO caps is deferred: a task that receives a
  MmioRegion cap can map to its own address space (which is correct), but
  the cap-broker-enforced "this cap is only for task X" scope is pending RFC 040.
- DMA quarantine timeout (RFC 036 §"Quarantine"): synchronous zeroize is used;
  the timer-callback path for device-quiesce uncertainty is `DEFERRED`.
- `dma_share` (nr 111): no capability check — still `Missing` (no use case).

## [0.2.0-alpha.3] - 2026-05-17

### v0.2 Phase 2 — Blocked IPC Revocation Semantics (RFC 034)

### Added

- **`fjell-ipc/src/endpoint.rs`** rewritten (RFC 034):
  - `PendingMessage.lease: Option<LeaseBinding>` — the sender's endpoint
    cap lease binding at send/call time.
  - `RecvWaiter { tid, lease }` replaces bare `u16` in `recvq`.
    Create with `RecvWaiter::no_lease(tid)` or `RecvWaiter::with_lease(tid, lb)`.
  - `Endpoint::cancel_by_lease(lease_id, epoch) -> CancelledByLease` — removes
    all sendq/recvq entries whose lease binding matches; returns cancelled TIDs.
  - `CancelledByLease { senders(), receivers() }` result type.
  - `EndpointError::LeaseRevoked` variant.
  - 10 unit tests (5 existing + 5 RFC 034 cases).
- **`fjell-ipc/src/reply.rs`** extended: `ReplyEdge.lease: Option<LeaseBinding>`.
  - `ReplyEdge::with_lease(caller_tid, lb)` constructor.
- **`fjell-kernel/src/cap/table.rs`**:
  - `set_reply_with_lease(server, caller, lease)` — stores lease in reply edge.
  - `cancel_replies_for_lease(lease_id, old_epoch)` — cancels blocked callers
    waiting for a reply whose call's lease was revoked; returns caller TIDs.
- **`fjell-kernel/src/cap/syscall.rs`**:
  - `sys_ipc_recv` passes `RecvWaiter::with_lease(tid, lb)` from the endpoint
    cap's lease binding.
  - `sys_ipc_call` stores lease in `PendingMessage` and uses
    `set_reply_with_lease`.
  - `sys_ipc_reply` checks the reply edge's lease; silently drops the reply with
    `LeaseRevoked` if the lease was revoked after the call was issued.
  - `cancel_blocked_ipc_for_lease(lease_id, old_epoch, ct, et, tasks, sched)`
    — the RFC 034 implementation: walks all endpoints and all reply edges,
    cancels matching entries, wakes cancelled tasks with `LeaseRevoked`.
  - `wake_with_error(tasks, sched, tid, e)` helper.
- **`fjell-kernel/src/trap/syscall.rs`** `dispatch_lease_revoke` now calls
  `cancel_blocked_ipc_for_lease` immediately after a successful revoke.

### Changed

- `fjell-ipc/src/lib.rs` exports `CancelledByLease`, `RecvWaiter`.
- Workspace version bumped to `0.2.0-alpha.3`.

### Known limitations (not yet closed)

- `wake_or_cancel_blocked_ipc_for_lease` in `lease/mod.rs` is still a no-op
  stub; the real implementation lives in `cap/syscall.rs` and is called
  directly by `dispatch_lease_revoke`. Phase 2 is functionally complete.

### Phase 2 negative tests (to be verified at QEMU build time)

- `NEG:IPC:BLOCKED_CALL_WAKES_ON_REVOKE:PASS`
- `NEG:IPC:BLOCKED_RECV_WAKES_ON_REVOKE:PASS`
- `NEG:IPC:LATE_REPLY_REJECTED:PASS`

## [0.2.0-alpha.2] - 2026-05-17

### v0.2 Phase 1 completion + Phase 2 foundation

Wires the Phase 1 enforcement library into the kernel, and delivers
the Phase 2 lease epoch revocation table.

### Added

- **`fjell-kernel/src/lease/mod.rs`** rewritten (RFC 033 Phase 2):
  - `LeaseState { Empty, Active, Revoked }` — explicit state enum.
  - `LeaseObject` gains `owner: TaskId` field for lifecycle revoke.
  - `create()` now starts epoch at `1` (RFC 033 §2.3: `0` is reserved).
  - `revoke()` is O(1): increments epoch + marks `Revoked` + calls
    `wake_or_cancel_blocked_ipc_for_lease` hook (RFC 034 stub).
  - `check_active()` returns `SysError::LeaseRevoked` (not PermissionDenied).
  - `revoke_owned_by(task)` — lifecycle revoke for all leases owned by a
    task; called on task exit/fault/restart.
  - `wake_or_cancel_blocked_ipc_for_lease` stub (RFC 034 hook, Phase 2).
  - 7 unit tests: `LEASE-001` through `LEASE-006` invariants verified.
- **`fjell-cap/src/rights.rs`**: `impl From<CapError> for SysError` — allows
  kernel code to use `?` on `check_lease` / `require_cap` results.

### Changed

- **`fjell-kernel/src/cap/syscall.rs`**:
  - `CapRights(tf.gpr[12] as u32)` → `CapRights(tf.gpr[12] as u64)`.
  - All `cap.check_lease(lt)?` and `err(tf, e)` → `err(tf, e.into())`.
  - `check_right` uses `.map_err(SysError::from)?`.
  - `sys_audit_drain` rights check corrected from `RECV` to `AUDIT_DRAIN`.
  - New function: `sys_cap_drop` (RFC 032 kernel entry point).
- **`fjell-kernel/src/trap/syscall.rs`**:
  - `CapDrop` added to the syscall dispatch table (calls `sys_cap_drop`).
  - Task/lease `require_cap` calls now use specific rights (TASK_CREATE,
    TASK_START, TASK_STATUS, LEASE_CREATE, LEASE_REVOKE, LEASE_INSPECT)
    instead of the broad `CapRights::ALL`.
  - Local `require_cap` comment clarifies it is a transition aid;
    V02-A-001 tracks migration to handle-based enforcement.
- **`fjell-kernel/src/audit/ring.rs`**: `CapDrop = 15` audit kind added.
- **`fjell-kernel/src/main.rs`**:
  - All `Capability { ... }` literals updated: `state: CapState::Active,
    scope: ObjectScope::Any` fields added.
  - Import extended: `use fjell_cap::{CapKind, CapRights, CapState, ObjectScope}`.
- **`fjell-kernel/src/lease/mod.rs`**: `LeaseChecker` impl now returns
  `CapError::LeaseRevoked` instead of `SysError::PermissionDenied`.

### Known limitations (not changed from alpha.1)

- The task/lease syscalls still use a CSpace slot-scan for capability
  checking. Full handle-based `require_cap` at each call site is tracked
  as V02-A-001 (requires ABI changes to pass the handle in `a0`/`a2`).
- `wake_or_cancel_blocked_ipc_for_lease` is a no-op stub. Blocked IPC
  wake/cancel lands when the Phase 2 CallFrame epoch-tracking data
  structures are added (RFC 034 completion).

## [0.2.0-alpha.1] - 2026-05-17

### v0.2 Phase 1 — Capability Enforcement Core (RFC 031, RFC 032)

First implementation drop of v0.2.0 *Security Boundary Closure*.
Contains the pure-logic enforcement infrastructure; kernel syscall
migration (wiring every call site to `require_cap`) is subsequent work
requiring QEMU testing.

### Added

- **`fjell-cap` — new types (RFC 031 §2.1–§2.7)**:
  - `CapRights` extended from `u32` to `u64` with 26 named bits
    (READ, WRITE, EXECUTE, SEND, RECV, CALL, REPLY, COPY, MINT,
    REVOKE, INSPECT, DROP, TASK_CREATE, TASK_START, TASK_STATUS,
    TASK_KILL, LEASE_CREATE, LEASE_REVOKE, LEASE_INSPECT, MMIO_MAP,
    DMA_ALLOC, DMA_USE, DMA_REVOKE, AUDIT_DRAIN, BOOT_READ, REBOOT).
  - `CapKind` extended with all RFC 031 variants (MmioRegion,
    DmaRegion, AuditDrain, BootEvidence, Reboot, PersistentStore,
    BootControl, UpgradeTransaction, Verification, RootfsRead,
    SnapshotCreate, SnapshotRead, TaskInspect, TaskCreate).
  - `CapState` — Active / Dropped / Revoked.
  - `ObjectScope` — Any, Task, Endpoint, Lease, MmioRegion, DmaRegion,
    Object, StoreNamespace, BootSlot.
  - `CapError` — 12-variant typed enforcement error with
    `to_sys_error()` mapping.
  - `CapSlotState` — Empty / Active / Dropped (RFC 032 §2.1).
  - `Capability` gains `state: CapState` and `scope: ObjectScope`.
  - `CapSlot` gains explicit `CapSlotState`.
  - `NoLease` / `AlwaysRevoked` test helpers.

- **`fjell-cap/src/enforcement.rs`** — new module (RFC 031 §2.5):
  - `require_cap(cspace, handle, expected_kind, required_rights,
    required_scope, checker)` — unified 7-step enforcement function
    (normative check order: lookup → generation → state → kind →
    rights → scope → lease).
  - `cap_drop(cspace, handle)` — explicit slot release (RFC 032 §2.4);
    succeeds on revoked-lease caps.

- **`fjell-cap` unit tests** — 16 tests covering all 7 check steps and
  the `cap_drop` invariants (NEG:CAP:MISSING_RIGHT, WRONG_KIND,
  GENERATION_MISMATCH, SCOPE_MISMATCH, REVOKED_LEASE, NULL_HANDLE,
  EMPTY_SLOT; DROPPED_HANDLE, STALE_AFTER_DROP, DROP_REVOKED_CAP,
  CSpace_REUSE_AFTER_DROP, DROP_NULL_REJECTED, MINT_RIGHTS_AMPLIFICATION).

- **`fjell-abi`**:
  - `SyscallNumber::CapDrop = 15` (RFC 032 §2.3).
  - `SysError::LeaseRevoked = -40`, `LeaseExpired = -41`,
    `GenerationMismatch = -42` (RFC 031 §2.7).

- **`fjell-syscall`**: `sys_cap_drop(cap: CapHandle) -> Result<(), SysError>`.

### Changed

- `CapRights` inner type changed from `u32` to `u64`.
  Old constants `GRANT`, `MAP_R`, `MAP_W`, `MAP_X` removed;
  use `SEND/RECV/CALL/COPY/MINT/READ/WRITE/EXECUTE` instead.
  Service crates that embed raw `u32` constants are noted as needing
  migration (see `docs/src/audit/capability-lease-enforcement-audit-v0.1.md`).
- `Capability::derive()` signature extended with `new_scope` and
  `self_slot` parameters.
- Workspace version bumped to `0.2.0-alpha.1`.

### Deferred to subsequent Phase 1 work

- Kernel syscall entry migration: replacing `caller_has_cap(kind)` with
  `require_cap()` in every syscall path (`fjell-kernel`). Requires QEMU
  cross-compile and smoke-test verification.
- `fjell-cap-broker`, `fjell-init`, `fjell-auditd`, `fjell-storaged`
  migration to new `CapRights` constants (v0.2 service crate work).

### Known Limitations

This alpha implements the enforcement *library*. The kernel is not yet
calling `require_cap()` at syscall boundaries — enforcement gaps listed
in `docs/src/audit/capability-lease-enforcement-audit-v0.1.md` remain
until the kernel migration lands.

### Deferred to Phase 2 (RFC 033, RFC 034)

- Lease epoch revocation connected to every use site.
- Blocked-IPC wake/cancel on lease revoke.

## [0.1.5] - 2026-05-17

### v0.1.x stabilization — v0.2 preparation backlog

### Added

- **RFC 047** (`rfcs/047-v02-preparation-backlog.md`, RFC-v0.1.x-011)
  — v0.2 preparation backlog.
- `docs/src/roadmap/v0.2-preparation-backlog.md` — 30 backlog items
  (18 release blockers) grouped by v0.2 epic (A–G), each with severity,
  source RFC, resolving RFC, required negative tests, and acceptance
  criteria.

### Changed

- Workspace version bumped `0.1.4 → 0.1.5`.
- `docs/src/SUMMARY.md` updated (v0.2 backlog link).

### Deferred to v0.2

Everything in the backlog document.

---

## [0.1.4] - 2026-05-17

### v0.1.x stabilization — ADR sync + release checklist

### Added

- **RFC 045** (`rfcs/045-adr-and-documentation-synchronization.md`,
  RFC-v0.1.x-009) — ADR and documentation synchronization.
- **RFC 046** (`rfcs/046-v01x-release-checklist.md`,
  RFC-v0.1.x-010) — v0.1.x release checklist.
- **12 ADRs** (`docs/src/adr/0001`–`0012`) per the mandated list:
  0001 Minimal Microkernel, 0002 Capability-Based IPC,
  0003 Lease Epoch Revocation, 0004 User-Space Service Plane,
  0005 Semantic Stream First, 0006 User-Space Driver Model,
  0007 Append-Only State Store, 0008 Verified Immutable Rootfs,
  0009 A/B Boot Control and Health Confirmation, 0010 Local Evidence
  and Recovery, 0011 Development-Grade Crypto Before Hardware Trust,
  0012 No General Network Before Security Closure.
  Each ADR includes the required fields: Status, Context, Decision,
  Consequences, Security Boundary Impact, Deferred Work, Related RFCs.
- `docs/src/adr/ADR-RENAME.md` — migration note from old filenames
  to new mandated filenames.
- `docs/src/releases/v0.1.x-release-checklist.md` — complete
  release gate checklist (build, test, doc, artefact, CHANGELOG rubric,
  version bump procedure, post-release steps).
- Old ADRs marked `**Status:** Superseded` with forward links.

### Changed

- Workspace version bumped `0.1.3 → 0.1.4`.
- `docs/src/SUMMARY.md` updated (ADR index + release checklist).

---

## [0.1.3] - 2026-05-17

### v0.1.x stabilization — Capability / Lease / MMIO / DMA / Evidence audits

### Added

- **RFC 029** (`rfcs/029-capability-lease-enforcement-audit.md`,
  RFC-v0.1.x-006) — capability / lease enforcement audit.
- **RFC 030** (`rfcs/030-mmio-dma-boundary-audit.md`,
  RFC-v0.1.x-007) — MMIO / DMA boundary audit.
- **RFC 044** (`rfcs/044-audit-snapshot-semantic-evidence-audit.md`,
  RFC-v0.1.x-008) — audit / snapshot / semantic evidence audit.
- `docs/src/audit/capability-lease-enforcement-audit-v0.1.md` —
  classification of all 29 syscall paths (1 OK, 18 Partial, 4 Missing,
  3 DebugOnly). Includes v0.2 enforcement checklist.
- `docs/src/audit/mmio-dma-boundary-audit-v0.1.md` — MMIO/DMA threat
  analysis, 5 Release Blocker gaps (all resolving to RFCs 035, 036),
  required negative-test markers.
- `docs/src/audit/evidence-export-audit-v0.1.md` — full 17-event
  evidence matrix mapping to audit/store/snapshot/semantic channels.
  7 critical gaps (no channels at all) and 6 partial gaps identified.
  Normative post-v0.2 target matrix.

### Changed

- Workspace version bumped `0.1.2 → 0.1.3`.
- `docs/src/SUMMARY.md` updated (Audits section).

---

## [0.1.2] - 2026-05-17

### v0.1.x stabilization — Negative test harness, threat model, ABI inventory

### Added

- **RFC 026** (`rfcs/026-negative-test-harness.md`, RFC-v0.1.x-003)
  — negative test harness.
- **RFC 027** (`rfcs/027-threat-model-and-security-boundaries.md`,
  RFC-v0.1.x-004) — threat model and security boundary documentation.
- **RFC 028** (`rfcs/028-syscall-abi-protocol-inventory.md`,
  RFC-v0.1.x-005) — syscall / ABI / protocol inventory.
- `docs/src/development/negative-tests.md` — complete negative-test
  catalogue with marker naming convention, per-category tables showing
  testability at v0.1.x vs. deferred to v0.2, and CI integration notes.
- `docs/src/security/threat-model-v0.1.md` — full 14-section threat
  model (supersedes v0.1.1 skeleton): assets, TCB, attacker model,
  trust boundaries, per-boundary enforcement, known weaknesses, deferred
  threats, v0.2 plan.
- `docs/src/abi/v0.1-inventory.md` — complete ABI inventory: 29
  syscalls with number/name/registers/required-cap/enforcement/stability,
  error code table, bootstrap ABI, service image ID table, IPC protocol
  inventory, persistent format inventory, semantic schema inventory.
- `tests/qemu/profiles/store.toml` — real markers for store corruption /
  recovery rejection (testable at v0.1.x).
- `tests/qemu/profiles/upgrade.toml` — real markers for signature
  verification rejection (testable at v0.1.x).

### Changed

- Workspace version bumped `0.1.1 → 0.1.2`.
- `docs/src/SUMMARY.md` updated (Security, ABI Reference, Development).

### Known Limitations

All v0.1.x limitations documented in `releases/v0.1.0-limitations.md`
apply. The threat model full body (§6–§11) replaces the skeleton but does
not close any enforcement gap — that is v0.2 work.

## [0.1.1] - 2026-05-17

### v0.1.x stabilization — Release freeze + CI foundation

This is the first stabilization release in the v0.1.x line.  It adds
no new OS functionality.  It freezes the v0.1.0 prototype, documents
its limitations, lays down the CI / negative-test infrastructure, and
files the v0.2 design RFCs so v0.2 can begin with a coherent plan.

### Added

- **RFC set 024–047** in `rfcs/`:
  - 024 (RFC-v0.1.x-001) — release freeze and scope declaration *(Accepted)*.
  - 025 (RFC-v0.1.x-002) — CI / QEMU automation foundation *(Accepted)*.
  - 026 (RFC-v0.1.x-003) — negative-test harness *(Proposed)*.
  - 027 (RFC-v0.1.x-004) — threat model and security boundaries *(Proposed)*.
  - 028 (RFC-v0.1.x-005) — syscall / ABI / protocol inventory *(Proposed)*.
  - 029 (RFC-v0.1.x-006) — capability / lease enforcement audit *(Proposed)*.
  - 030 (RFC-v0.1.x-007) — MMIO / DMA boundary audit *(Proposed)*.
  - 031–043 (RFC-v0.2-001..013) — full v0.2 *Security Boundary Closure*
    RFC set *(Proposed)*: unified capability enforcement, CSpace GC,
    lease epoch revocation, blocked-IPC wake/cancel, MmioRegion ABI
    replacement, DmaRegion zeroize/quarantine, non-blocking IPC + timer
    fail-safe, service-plane separation, safe user copy + real audit
    drain, cap-broker bootstrap handoff and default deny, persistent
    evidence hardening, v0.2 negative-test expansion, v0.2 security
    boundary release gate.
  - 044 (RFC-v0.1.x-008) — audit / snapshot / semantic evidence audit
    *(Proposed)*.
  - 045 (RFC-v0.1.x-009) — ADR and documentation synchronization
    *(Proposed)*.
  - 046 (RFC-v0.1.x-010) — v0.1.x release checklist *(Proposed)*.
  - 047 (RFC-v0.1.x-011) — v0.2 preparation backlog *(Proposed)*.
- **Documentation** under `docs/src/`:
  - `releases/v0.1.0-scope.md` — what v0.1.0 includes.
  - `releases/v0.1.0-limitations.md` — what v0.1.0 is *not* (no
    production secure boot, no remote attestation, no networking, no
    POSIX, etc.).
  - `security/v0.1.0-known-non-goals.md` — non-goals contributors
    must not extend into.
  - `security/v0.1.0-threat-model.md` — skeleton; full body lands
    with RFC 027 in v0.1.2.
  - `roadmap/v0.1.x-stabilization.md` — v0.1.1 → v0.1.5 sequence.
- **`fjell-tools` xtask extensions** (RFC 025):
  - `cargo xtask qemu-negative <category>` — runs a profile-driven
    negative test under `tests/qemu/profiles/`.
  - `cargo xtask qemu-log-check <log-file> <marker>` — generic
    substring-match validator.
  - `cargo xtask qemu-run --profile <name>` — explicit profile runner.
  - All QEMU runs now write to `tests/qemu/artifacts/<run-id>/` with
    `serial.log`, `qemu-command.txt`, `expected-markers.txt`, and
    `result-summary.txt`.
- **Placeholder profile TOMLs** for the six v0.1.x negative-test
  categories (`capability`, `ipc`, `mmio`, `dma`, `store`, `upgrade`).
  Each profile asserts no markers yet — they are real PASSes
  *infrastructure-wise* per RFC 025 §"chicken-and-egg" exemption; case
  bodies land per v0.2 RFC.
- **`.github/workflows/ci.yml`** with five jobs (`ci-format`,
  `ci-check`, `ci-test-host`, `ci-qemu-smoke`, `ci-qemu-negative`),
  matrix-parameterised over milestones / categories, with artefact
  upload.

### Changed

- `README.md` updated: version stamp v0.0.2 → v0.1.1, prominent
  limitation warning block linking to
  `docs/src/releases/v0.1.0-limitations.md`.
- `ROADMAP.md` updated: replaced placeholder v0.2–v0.4 stub with the
  full v0.1.x stabilization table, v0.2 nine-phase plan, and v0.3
  through v1.0 progression.
- `docs/src/SUMMARY.md` updated: new top-level sections *Releases*,
  *Roadmap*, *Security* preceding *Getting Started*.
- `crates/fjell-tools/src/main.rs` rewritten to dispatch the four
  RFC-025 subcommands.
- `crates/fjell-tools/src/smoke.rs` refactored to use the shared
  `Profile` / `run_profile` runner; semantics preserved
  (TEST:Mx:PASS marker map unchanged).
- Workspace version bumped `0.1.0 → 0.1.1`.

### Fixed

- *(none — this release intentionally adds no OS functionality)*

### Security

- No security-boundary changes in v0.1.1 itself. The v0.2 RFC set
  (RFCs 031–043) defines every boundary closure that will land in
  v0.2.0.
- Threat-model and limitations are now explicit project documents
  rather than implicit assumptions.

### Known Limitations

All limitations documented in
`docs/src/releases/v0.1.0-limitations.md` apply unchanged to v0.1.1.
In particular: no production secure boot, no hardware-rooted trust,
no remote attestation, no networking, no POSIX, no GUI, no fully
verified components, no uniform security-boundary enforcement.

### Deferred to v0.2

- Implementation of every RFC-v0.2 design (RFCs 031–043).
- Replacing `caller_has_cap` style checks with `require_cap`.
- O(1) lease epoch revocation across syscall and IPC paths.
- Blocked-IPC wake/cancel on revoke.
- MmioRegion / DmaRegion capability ABIs.
- DMA zeroize / quarantine.
- `sys_ipc_try_recv` + cooperative service loops + timer fail-safe.
- Real service-plane separation (ADR-0010 supersession).
- Safe `copy_to_user` + real audit drain (binary AuditRecord).
- `cap-broker` bootstrap handoff and default-deny policy engine.
- Persistent evidence hardening matrix.
- v0.2 negative-test expansion and v0.2 release gate.

## [0.1.0] - 2026-05-17

### M8 completion — Local Evidence / Attestation / Recovery Plane

This is the v0.1.0 release of Fjell OS, completing all eight milestones (M0-M8).

#### PR-M8-07: Semantic stream integration

- `fjell-semantic-format` extended with M8 StateKind (MeasurementStatus,
  AttestationStatus, BundleFreshnessStatus, RecoveryStatus), EventKind
  (BundleFreshnessRejected, RollbackSelected, etc.), and ActionKind
  (SelectRollback, InspectSnapshots).

- `fjell-proxy-text` extended with M8 render helpers: `render_measurement_status`,
  `render_attestation_status`, `render_freshness_status`, `render_recovery_status`,
  `render_recovery_intent`, `render_freshness_rejected_event`,
  `render_rollback_selected_event`.

- `fjell-init` now publishes the four M8 StateNodes during the smoke run:
  `[STATE][Ok] Measurement status`, `[STATE][Ok] Attestation status`,
  `[STATE][Ok] Bundle freshness`, `[STATE][Ok] Recovery status`.
  RecoveryIntent IntentNode is rendered with Inspect/Rollback actions.

#### PR-M8-08/09: Negative path smoke and hardening

- Negative path (generation-rollback rejection to recovery target) exercised
  inline. A bundle with gen=3 against last_accepted_gen=5 is rejected.
  `[EVENT][Important][Failed] Bundle freshness rejected` and
  `[STATE][Failed] Bundle freshness` are published. `recoveryd` ENTER_RECOVERY
  is called; `[STATE][Ok] Recovery status` and RecoveryIntent are re-published.

#### v0.1.0 Acceptance criteria

All M8 acceptance criteria from the internal design document (sections 20.1-20.8)
are met. `TEST:M8:PASS` and `TEST:M7:PASS` are emitted. 24 format-crate unit
tests pass covering SHA-256 correctness, chain-digest determinism, attestation
tamper detection, and all freshness rejection paths.

## [0.0.15] - 2026-05-17

### Added (M8: Local Evidence / Attestation / Recovery Plane)

- **PR-M8-01 — Format crates**: three new `no_std` library crates establish
  the M8 data model.
  - `fjell-measure-format`: append-only measurement chain types.  Implements
    a compact SHA-256 (no external crate) for deterministic chain-digest
    computation: `chain_digest[n] = SHA256(domain || seq || kind || source ||
    subject || subject_digest || metadata_digest || prev_chain_digest)`.
    `MeasurementEvent`, `MeasurementHead`, `MeasurementKind/Source/Subject`
    enums.  24 unit tests pass including SHA-256 known-answer vectors,
    determinism, chain-linkage, and metadata influence.
  - `fjell-attestation-format`: `AttestationRecord`, `SignedAttestationRecord`,
    `DevAttestation` (development-grade Ed25519 stand-in via SHA-256 keyed
    under `dev-attest-m8-01`).  Canonical digest covers all claims fields.
    Tamper-detection unit tests confirm that modified records fail verify.
  - `fjell-recovery-format`: `RecoveryRequest/Response`, `BundleMetadataV2`,
    `FreshnessCheck`, `FreshnessStatus`, `SlotInspection`, rollback types.
    `BundleMetadataV2::check_freshness` enforces validity window, generation
    anti-rollback, and key-epoch anti-rollback.  8 unit tests cover all
    rejection paths (expired, not-yet-valid, generation rollback, key-epoch
    rollback, unsupported schema).

- **PR-M8-02 — Service API extension**: `fjell-service-api` extended with
  `measuredd`, `attestd`, `recoveryd`, and `verifyd` protocol modules
  (tags 0x300–0x33F).  Backward-compatible with all M7 tags (0x200–0x215).

- **PR-M8-03 — measuredd**: new `fjell-measuredd` service.  Maintains an
  in-memory append-only measurement chain (up to 64 events, ring buffer).
  Accepts `APPEND_EVENT` (kind | source | subject | digest), replies with
  `APPEND_OK` carrying the new sequence number and chain-digest prefix.
  `GET_HEAD` returns current chain state.  Announces readiness on private
  endpoint 2.

- **PR-M8-04 — verifyd freshness**: bundle freshness validation implemented
  in `fjell-recovery-format::BundleMetadataV2::check_freshness` and exercised
  in `fjell-init`.  Valid bundle (gen=5, epoch=3, tick∈[1000,9000]) is
  admitted.  Expired bundle (tick=9999) and generation-rollback bundle
  (last_gen=6 > gen=5) are both rejected, satisfying FRESH-INV-001 through
  FRESH-INV-003.

- **PR-M8-05 — attestd**: new `fjell-attestd` service.  Generates local
  development-grade attestation records from measurement chain state.  Signs
  with `DevAttestation` (SHA-256 keyed under `dev-attest-m8-01`).  Reports
  `GENERATED` and passes `VERIFY_LATEST` check in the smoke test.

- **PR-M8-06 — recoveryd**: new `fjell-recoveryd` service with
  `LIST_SNAPSHOTS`, `INSPECT_SLOT`, `ENTER_RECOVERY`, `SELECT_ROLLBACK`, and
  `EXPORT_DIAGNOSTICS` handlers.  Enforces `REC-001`: `SELECT_ROLLBACK`
  without `confirmed_by_operator=true` returns `ERR::NotConfirmed`.  Confirmed
  rollback returns `ROLLBACK_SELECTED`.

- **Kernel endpoint allocation**: endpoints 2, 3, 4 pre-allocated in
  `main.rs` for measuredd, attestd, recoveryd respectively.  `spawn.rs`
  updated to assign private endpoint IDs per service.  Init's CSpace gains
  slots 3–5 for the M8 service endpoints.

- **M8 smoke test**: `cargo xtask qemu-test m8` now emits `TEST:M8:PASS`
  after exercising all six M8 acceptance criteria: (1) boot evidence import,
  (2) verification result append, (3) freshness valid path, (4) expired-bundle
  rejection, (5) generation-rollback rejection, (6) attestation generation and
  verification, (7) unconfirmed-rollback rejection, (8) confirmed-rollback
  acceptance.  `TEST:M7:PASS` is preserved.

## [0.0.14] - 2026-05-17

### Added (RFC 019, storaged / virtio-blk IPC completion)

- **storaged: virtio-blk device discovery** — corrected QEMU command from
  `-drive if=virtio` (which creates `virtio-blk-pci` on PCIe, leaving all
  eight virtio-mmio buses empty) to
  `-drive file=…,if=none,id=hd0 -device virtio-blk-device,drive=hd0`.
  storaged now scans virtio-mmio buses 0–7 for DeviceID = 2 and correctly
  finds the block device placed by QEMU on bus 1 (bus 0 = virtio-rng).

- **storaged: virtio v1 legacy init ordering** — DRIVER_OK is now written
  *before* QueueNum / QueuePFN so that `virtio_bus_start_ioeventfd` fires
  while `vring.num == 0` (default after reset = 256, but the guest has not
  yet written QueueNum).  This prevents the ioeventfd from being registered
  at offset 0x050, which would silently intercept every subsequent
  QueueNotify write and bypass `virtio_mmio_write`.  All writes therefore
  take the synchronous direct-call path through `virtio_queue_notify`.

- **IpcReply ABI fix** — `reply(tag)` in storaged was placing the reply
  tag in `a0`; the kernel's `sys_ipc_reply` reads the reply label from
  `a1` (`gpr[REG_A1]`).  Fixed to `in("a1") tag`, `in("a0") 0`.  Without
  this fix every `WRITE_OK` reply delivered garbage to init, causing all
  `storaged_write` calls to return `false` even though the virtio I/O
  completed correctly.

- **M7 smoke test passes end-to-end** — `TEST:M7:PASS` is now reliably
  emitted.  All six `storaged_write` calls (sector 193, LBA_SUPERBLOCK_A,
  LBA_LOG_START, LBA_SUPERBLOCK_A×2, LBA_BOOT_CTL_A_START,
  LBA_BOOT_CTL_B_START) succeed; `virtio_blk_req_complete status=0` is
  confirmed for each via QEMU trace.

### Fixed

- `fjell-tools` smoke runner: QEMU command updated to use
  `virtio-blk-device` (virtio-mmio transport) instead of the implicit
  `virtio-blk-pci` created by `-drive if=virtio`.

## [0.0.13] - 2026-05-16

### Added (RFC 020, 021, 016, 017)

- **RFC 020 — Audit drain** (`sys_audit_drain`): kernel audit ring is now
  fully drained by `auditd` via a capability-gated syscall.  `AuditRing` gains
  a `drain_cursor` + `compact()` so consumed slots are reclaimed.  A new
  `copy_to_user_bytes` helper translates user VA → PA through the Sv39 page
  table and writes via the kernel identity map.  `AuditRecordBin` (32-byte
  flat struct) and `AuditKind::label()` are added to `fjell-audit-format`.
  `auditd` emits one JSON Lines record per kernel event on startup and on
  each IPC signal.

- **RFC 021 — cap-broker real policy evaluation**: replaced the tag-byte stub
  with a three-pass evaluator (explicit Deny → explicit Allow → default Deny)
  matching RFC requirements BROKER-001 through BROKER-008.  `ResourceClass`,
  `PolicyKind`, `PolicyResult`, and `PolicyRule` are now proper types.
  Granted capabilities are lease-bound via `sys_lease_create`.  The IPC
  protocol is extended to carry `(requester_id, resource_class, requested_rights)`
  as three IPC words.  `sys_ipc_recv_msg` added to `fjell-syscall` to return
  all five values (label + 4 words).

- **RFC 017 — DmaAlloc capability gate**: `sys_dma_alloc` now requires the
  caller to hold `CapKind::DmaAlloc` (CSpace slot 2).  Granted to
  `fjell-storaged` and `fjell-driver-virtio-blk` at spawn time.  `release_task`
  now also frees (returns to allocator) the physical DMA frame after zeroing it,
  preventing frame leaks on task exit.

- **RFC 016 confirmed complete**: `sys_mmio_map` cap-gate, bounds-check against
  `MmioRegionTable`, and defense-in-depth RAM exclusion were already fully
  implemented in v0.0.11/v0.0.12; this release documents the confirmation.

### Changed

- `sys_dma_alloc` ABI: `(size_bytes)` → `(dma_cap_handle, size_bytes)`.
  Callers must pass the `CapKind::DmaAlloc` handle as `a0`; size moves to `a1`.
- `sys_audit_drain` ABI: `(buf_ptr, buf_len)` → `(buf_va, buf_cap, cap_handle)`;
  returns `(status, n_records, n_dropped)`.

## [0.0.12] - 2026-05-14

### Added
- RFC 019: `storaged` IPC service separation — virtio-blk block I/O is now
  owned by the `storaged` service; `fjell-init` communicates with it via
  structured IPC (WRITE_BEGIN / WRITE_CHUNK×16 / WRITE_COMMIT protocol).
- M7 smoke test (`TEST:M7:PASS`): verifies end-to-end IPC block writes,
  store format operations, and boot-slot confirmation simulation.

### Fixed
- `wait_storaged_ready`: declared `a2`–`a5` as asm clobbers so the compiler
  does not cache the READY constant in `a2` across the `ecall` (which
  `deliver()` always overwrites with `sender_badge = 0`).
- `ipc_call` / `wait_storaged_ready` / storaged IPC wrappers: declared `a7`
  as an asm clobber for all `"li a7, N", "ecall"` blocks.  Without this, the
  compiler allocated `a7` as a live loop variable (e.g. the chunk pointer in
  `storaged_write`); the kernel restores `a7 = ecall_nr` from the trapframe on
  wake, corrupting the variable.
- Removed duplicate `spawn(ImageId::STORAGED)` call that created a second
  storaged instance competing on endpoint 1.
- `sys_ipc_reply`: now reads the reply label from `a1` (was incorrectly
  reading from `a0 = 0`) and copies reply words to the caller's trapframe.

---

## [Unreleased]

---

## [0.0.11] — 2026-05-12 — M7.1 Security Hardening Round 2 (RFC 014, 015, 018, 022)

Third batch of architect-review-driven fixes.  Implements RFC 014, 015, 018, 022.
RFCs 016, 017, 019–021, 023 are specified and accepted; implementation deferred to
M8 or requires service separation (RFC 019) as prerequisite.

### Fixed / Added

- **RFC 014** (`trap/syscall.rs`): Replaced `caller_has_cap(kind)` with
  `require_cap(kind, rights)` — validates CapKind, CapRights, AND lease liveness.
  Added missing capability gates: `sys_task_status` now requires
  `TaskControl | INSPECT`; `sys_lease_revoke` and `sys_lease_inspect` now require
  `LeaseAdmin`.  All 6 task/lease syscalls are gated (RFC 004 gated only 3).

- **RFC 015** (`cap/syscall.rs`): Lease validation wired into all capability check
  paths.  `check_right()` (IPC send/recv/call) calls `cap.check_lease()`.
  `sys_cap_copy` and `sys_cap_mint` validate source cap lease before derivation.
  `sys_cap_inspect` validates lease before returning metadata.  Revoked caps now
  fail all IPC and cap operations.

- **RFC 018** (`link.ld`, `main.rs`): W^X three-region kernel permissions achieved.
  Linker script adds `ALIGN(4096)` between .text / .rodata / .data / .bss and
  includes RISC-V orphan sections (.srodata, .sdata, .sbss, .got).  The map loop
  now uses three regions: `.text=R|X`, `.rodata=R`, `.data/.bss/stack=R|W`.
  Section starts are page-aligned; no page straddles two permission regions.
  Verified: `__rodata_start=0x80005000`, `__data_start=0x8000c000` (both page-aligned).

- **RFC 022** (`trap/syscall.rs`): `sys_task_start` validates `entry_pc` and
  `stack_top` against user address range (`[0x1000, RAM_BASE)` and `[0x2000, RAM_BASE)`
  respectively).  Kernel addresses are rejected with `InvalidCap`.

### Accepted / Deferred

RFC 016 (MmioRegion cap), RFC 017 (DmaRegion cap), RFC 019 (try_recv + service loop),
RFC 020 (audit drain), RFC 021 (cap-broker policy), RFC 023 (BCB mirror tests) — all
specified in `rfcs/`; implementation targets M8 or requires RFC 019 as prerequisite.

---

## [0.0.10] — 2026-05-12 — M7.1 hardening (RFC 006, 007, 009, 013)

Second batch of architect-review-driven fixes.  Implements RFC 006, 007, 009, 013.
Remaining deferred RFCs: 011 (service separation), 012 (real crypto) — both require
M8 preemptive scheduler as prerequisite.

### Added / Changed

- **RFC 006** (`fjell-cap/src/slot.rs`, `fjell-kernel/src/lease/mod.rs`):
  `LeaseBinding { lease_id, epoch_at_issue }` added to `Capability` (field `lease:
  Option<LeaseBinding>`).  `Capability::check_lease(&dyn LeaseChecker)` validates
  lease liveness.  `LeaseChecker` trait defined in `fjell-cap`; `LeaseTable` implements
  it in the kernel.  All capability constructors (`install_root`, `install_raw`,
  `derive`, bootstrap literals) set `lease: None` for unbound caps.  Infrastructure is
  in place; lease-bound delegation used by M8 cap-broker.

- **RFC 007** (`fjell-kernel/src/main.rs`, `trap/syscall.rs`): Replaced singleton
  `DMA_BUF` static with a per-task DMA VA bump allocator at `0x60000000+` (VPN[2]=1).
  `sys_dma_alloc` allocates frames from the frame allocator, maps them at
  `DMA_VA_NEXT` in the calling task's page table, and returns `(user_va, device_pa)`.
  VPN[2]=1 is task-local (not shared via `clone_kernel_half`), resolving the
  `AlreadyMapped` conflict from the M6 static-buffer approach.

- **RFC 009** (`crates/fjell-kernel/link.ld`, `main.rs`): W^X kernel page permissions.
  Linker script exports `__text_start`, `__text_end`, `__rodata_start`, `__rodata_end`.
  Kernel identity map now uses a two-region split:
  - `.text` (RAM_BASE .. __text_end): **R | X** — execute, not writable
  - everything else (.rodata / .data / .bss / stack): **R | W** — read-write, not executable
  No page is simultaneously writable and executable.  Full three-region split
  (.rodata = R only) deferred: requires confirming no writable statics in .rodata.

- **RFC 013** (`docs/src/adr/`): Created ADR 0006–0010 documenting all M6/M7 design
  decisions, workarounds, and deprecation plans:
  - ADR 0006: User-space driver model and MMIO/DMA capability boundary
  - ADR 0007: Persistent append-only store and recovery model
  - ADR 0008: Verified immutable rootfs and signed artifact model
  - ADR 0009: A/B boot-control and health-based confirmation model
  - ADR 0010: Inline init smoke workaround and service separation deprecation plan

---

## [0.0.9] — 2026-05-12 — M7.1 Security & Architecture Hardening

Implements RFC 004, 005, 008, 010 in response to architect review of v0.0.8.
RFCs 006, 007, 009, 011–013 are specified and accepted; implementation deferred
to M7.1 hardening sprint or M8.

### Fixed / Added

- **RFC 004** (`fjell-cap`, kernel): Added `CapKind::TaskCreate`, `CapKind::TaskControl`,
  `CapKind::LeaseAdmin`.  `sys_task_spawn` now requires `TaskCreate`; `sys_task_start`
  requires `TaskControl`; `sys_lease_create/revoke/inspect` require `LeaseAdmin`.
  init task receives all three as bootstrap capabilities at slot 28/29/30 in its CSpace.
  `CSpace::slots()` and `CSpace::install_raw()` added for kernel-internal use.

- **RFC 005** (`kernel/trap/syscall.rs`): `sys_mmio_map` rejects any request whose
  physical range overlaps kernel RAM (`RAM_BASE..RAM_END`).  Prevents user-space from
  mapping kernel text, data, or stack with R|W|U.

- **RFC 008** (`fjell-upgrade-format`, `fjell-store-format`): `BootControlBlock` and
  `StoreSuperblock` now have `seal()` (compute and store CRC32) and updated `is_valid()`
  (checks magic AND CRC32).  `fjell-init` calls `seal()` before writing BCB and
  superblocks to disk.  CRC32 uses ISO 3309 / Castagnoli (0xEDB88320) in `no_std`.
  Added 2 regression tests (seal + corrupt byte detection).

- **RFC 010** (`kernel/trap/syscall.rs`, `fjell-syscall`): `sys_task_spawn` now returns
  `handle = index | (generation << 16)`.  `sys_task_start` and `sys_task_status` decode
  the generation from the handle and look up the task with generation check, preventing
  stale handle reuse.

### Accepted / Deferred

RFC 006 (LeaseBinding in Capability), RFC 007 (per-task DMA), RFC 009 (W^X kernel
permissions), RFC 011 (service separation), RFC 012 (real digest verification),
RFC 013 (ADR 0006–0010) are fully specified in `rfcs/` with implementation deferred.

---

## [0.0.8] — 2026-05-12 — RFC bugfix release

Implements RFC 001, RFC 002, RFC 003 identified during M7 self-review.

### Fixed

- **RFC 001** (`trap/entry.rs`): t5 (x30) and t6 (x31) saved with wrong values at
  trap entry. `gpr[30]` received `user_sp`; `gpr[31]` received `scratch_addr`.
  Added `TRAP_SCRATCH[3]` (4th slot) to save the true user t6 immediately after the
  `csrrw t6, sscratch, t6` swap. `gpr[30]` is now saved directly after the TrapFrame
  pointer is loaded, while x30 is still unmodified. Both caller-saved registers are
  now faithfully preserved across ecalls.
  Removed 5 redundant `sys_mmio_map` re-read workaround calls from `fjell-init`
  that were masking the corruption.

- **RFC 002** (`fjell-upgrade-format/src/lib.rs`): `BootControlBlock::new()`
  initialised `slot_b` with `SlotInfo::bootable(generation)`. Slot B is unprovisioned
  on a fresh disk and must be `SlotInfo::empty()`. Confirmed already correct in current
  code; added 3 regression unit tests to prevent recurrence.

- **RFC 003** (`fjell-kernel/Cargo.toml`): `version = "0.0.3"` hard-pinned while
  workspace was at 0.0.7. Changed to `version.workspace = true`; kernel version now
  correctly tracks workspace in build output and `cargo metadata`.

---


## [0.0.7] — 2026-05-12 — M7: Verified Immutable System / Snapshot / Complete Rollback Foundation

### Added
- `fjell-verify-format`: `DevSignature`, `TrustAnchor`, `SignedObject`, `ObjectKind`,
  `VerificationResult`, `BootEvidence`, `ReleaseManifest`, `RootfsManifest`,
  `PolicyBundle`; development-grade Ed25519 placeholder with hardcoded `DevSignature::VALID`
- `fjell-rootfs-format`: `ServiceImageRef`, `RootfsNamespace`, `RootfsStatus`
- `fjell-snapshot-format`: `SnapshotId`, `SnapshotReason`, `SnapshotDigest`,
  `SystemSnapshot`; reasons: Boot, PreUpgrade, PostConfirmation, Rollback, Periodic
- `fjell-verifyd`, `fjell-rootfsd`, `fjell-snapshotd`: stubs (verification and
  snapshot logic driven inline by fjell-init for M7 smoke test)
- `ImageId::VERIFYD` (14), `ImageId::ROOTFSD` (15), `ImageId::SNAPSHOTD` (16)
- fjell-init M7 scenario:
  - Boot evidence loading (`BootEvidence::for_slot`, `TrustAnchor::DEV.is_valid()`)
  - Release manifest, rootfs manifest, policy bundle signature verification
    (`SignedObject::verify_dev()`)
  - Immutable rootfs namespace (`RootfsNamespace::empty() + add()`)
  - Pre-upgrade system snapshot (`SystemSnapshot::new(SnapshotReason::PreUpgrade)`)
  - Upgrade staging with verified bundle, slot marking, candidate set
  - Candidate boot simulation → health check → `slot confirmed after health`
  - Post-confirmation snapshot (`SnapshotReason::PostConfirmation`)
  - Semantic state export: `[STATE][Ok] Verified boot status`, `Immutable rootfs`,
    `System snapshot`, `[EVENT][Normal][Ok] Slot confirmed after health`
  - Negative test: `ReleaseManifest::invalid_dev` → signature rejected
  - Health failure rollback: `rollback selected as expected` + rollback snapshot
- `MAX_TASKS` increased from 16 to 32 to accommodate M4–M7 services (17 tasks)

### Fixed
- Task table overflow: M7 requires 17+ tasks; `MAX_TASKS=16` caused spawn failure
  at `fjell-rootfsd`; increased to 32 in `tcb.rs` and `scheduler.rs`

---

## [0.0.6] — 2026-05-12 — M6: Device / Persistent State / Immutable Upgrade Foundation

### Added
- `fjell-device-format`: `DeviceDescriptor`, `DeviceKind`, `MmioRegionDescriptor`,
  `DeviceState`; hardcoded `QEMU_VIRTIO_BLK` descriptor
- `fjell-block-format`: `BlockDeviceInfo`, `BlockError`
- `fjell-store-format`: `StoreSuperblock` (magic `FJSTORE\0`), `RecordHeader`
  (magic `0x464A4C52`), `RecordKind`, `LBA_*` layout constants
- `fjell-upgrade-format`: `BootControlBlock` (magic `FJBOOT\0\0`), `SlotInfo`,
  `SlotState`, `SlotId`, `UpgradeState`
- `fjell-devmgr`: calls `sys_platform_info_get`, emits "M6: virtio-mmio blk discovered"
- `fjell-driver-virtio-blk`: stub (virtio I/O done inline by fjell-init in M6)
- `fjell-storaged`, `fjell-bootctl`, `fjell-upgraded`, `fjell-powerd`: stubs
- Kernel syscalls: `sys_platform_info_get` (80), `sys_mmio_map` (90),
  `sys_dma_alloc` (110); all wired in dispatch table
- `sys_platform_info_get`: scans virtio-mmio slots 0..7 in reverse, returns base
  PA of first slot with magic=0x74726976 and device_id=2
- `sys_mmio_map`: calls `remap_page` to add R|W|U to an existing kernel-mode
  mapping; uses identity map (user_va == phys_addr)
- `sys_dma_alloc`: returns pre-allocated static `DMA_BUF` at user VA 0x20000000
- `DMA_BUF`: `#[repr(align(4096))]` 16 KiB static DMA buffer; mapped with R|W|U
  into init's page table during task creation (4 pages at 0x20000000-0x20004000)
- `remap_page`: new page-table primitive that overwrites existing PTEs
  (needed to upgrade R|W kernel-only mappings to R|W|U user-accessible)
- Kernel identity-maps all 8 virtio-mmio slots (0x10001000-0x10008000) with R|W
  in all user page tables so `sys_platform_info_get` can scan from kernel mode
- fjell-init M6: inline virtio-blk driver (legacy v1 init, split virtqueue,
  block write), storaged simulation, bootctl A/B mirror write, upgrade staging
- `cargo xtask build-services` and `cargo xtask qemu-test m6` with disk image
  creation (`qemu-img create -f raw fjell-disk.img 16M`)

### Fixed
- `sys_platform_info_get`: added VPN[2]=0 virtio-mmio mapping to ALL user page
  tables (kernel-mode mapping in spawn.rs + kmain init) so scan works from
  within the user task's satp context
- `blk_write_sector` `base` re-read after ecalls: local variable `base` holding
  the MMIO VA was potentially corrupted by t5/t6 register-save bug across ecalls;
  fixed by calling `sys_mmio_map` again to reload `base` from the syscall
- virtio-blk QueueAlign: changed from 4096 to 512 so desc+avail+used fit in
  one DMA page; with QueueAlign=4096 the used ring was at page+4096 which was
  beyond the mapped DMA region
- virtio-blk avail ring offset: was 0x200 (wrong ALIGN interpretation); corrected
  to 0x080 (= N*16 = 8*16 = 128, immediately after descriptor table)
- virtio-blk poll: increased from 500_000 to 5_000_000 iterations to handle
  QEMU subprocess execution slower than interactive mode
- `sys_dma_alloc`: replaced dynamic frame allocation + user-VA mapping with
  pre-mapped static DMA_BUF to avoid `AlreadyMapped` errors when kernel-shared
  L1 tables prevented adding U bit via `map_page`

---

## [0.0.5] — 2026-05-12 — M5: Semantic Operations Plane

### Added
- `fjell-semantic-format`: `IntentNode`, `StateNode`, `EventNode`,
  `SemanticEnvelope`, `TextToken`, `BoundedText`, `FixedVec<T,N>`,
  `ActionRequest`, `ActionResult`, `StreamFilter`, `ExportFormat`;
  invariant validators (`validate_intent`, `validate_state`);
  4 unit tests (valid intent, empty-action error, failed-state error,
  BoundedText roundtrip)
- `fjell-semantic-stream` (new service): publish/subscribe/validate/
  action dispatch; memory-backed intent/state/event rings
- `fjell-proxy-text` (full implementation): text rendering for
  `[STATE]`, `[EVENT]`, `[INTENT]` nodes; `SmokeScenario` auto-selects
  first action; no pixel/color/layout metadata required
- `fjell-init` (M5): full M5 smoke scenario — starts semantic-stream
  and proxy-text; publishes ServiceGraph, ConfigValidated, AuditSummary,
  CapabilityGranted, sample IntentNode; drives action dispatch;
  exports system state as plain text; emits `TEST:M5:PASS`
- `fjell-abi`: `ImageId::SEMANTIC_STREAM = 6`, `ImageId::PROXY_TEXT = 7`
- Kernel image table: `fjell-semantic-stream.bin`, `fjell-proxy-text.bin`
- `cargo xtask qemu-test m5` smoke gate

### Fixed
- Service linker script: stack start pinned to fixed VA `0x80000`
  (`__stack_top = 0x90000`) — previously `__stack_top` varied with
  binary size, causing `SERVICE_STACK_TOP` mismatches after the init
  binary grew to include semantic-format and proxy-text code
- `spawn.rs` + kmain: map all 16 stack pages (64 KiB) instead of one —
  init uses ~32 KB of stack for M5 scenario; single-page mapping caused
  `StorePageFault` at `0x87ec8`
- `Cargo.toml workspace.default-members`: trailing `]` was accidentally
  dropped by a prior edit, corrupting the TOML array

---

## [0.0.4] — 2026-05-12 — M4: Service Plane Bootstrap

### Added
- `fjell-abi`: `LeaseId`, `LeaseEpoch`, `BootInfo`, `ImageId`, `ServiceId`,
  `TaskLifecycle`; M4 syscall numbers (TaskSpawn=40, TaskStart=41,
  TaskStatus=42, TaskKill=43, LeaseCreate=50, LeaseRevoke=51,
  LeaseInspect=52, AuditDrain=60)
- `fjell-syscall`: complete user-space wrappers for all M3+M4 syscalls;
  service runtime module (`rt.rs`) with `_start` + `#[panic_handler]`
- `fjell-service-api`: IPC protocol tag constants for service lifecycle,
  config, cap-broker, audit, and service-manager protocols
- `fjell-cap-broker`: policy-evaluation service; static rule table;
  lease-create/revoke/inspect smoke demonstration
- `fjell-configd`: bootstrap manifest validation; config-get endpoint
- `fjell-auditd`: kernel audit ring drain; JSON Lines emission
- `fjell-service-manager`: service graph bootstrap; sample-service spawn
- `fjell-sample-service`: minimal ready/heartbeat/shutdown service
- `fjell-init` (full implementation): orchestrates configd → cap-broker →
  auditd → service-manager → sample-service; emits audit JSON Lines;
  prints `TEST:M4:PASS`
- `fjell-kernel/src/lease/`: `LeaseTable` with `create`/`revoke`/
  `check_active`/`current_epoch`; MAX_LEASES=32
- `fjell-kernel/src/task/image.rs`: embedded flat-binary service image table
  (`include_bytes!` of six release binaries); `SERVICE_BASE_VA=0x40000`,
  `SERVICE_STACK_TOP=0x51000`
- `fjell-kernel/src/task/spawn.rs`: flat-binary task spawner; multi-page
  text loading; UART + kernel-half mapping in spawned address space
- Kernel M4 syscall handlers: `sys_task_spawn`, `sys_task_start`,
  `sys_task_status`, `sys_lease_create`, `sys_lease_revoke`,
  `sys_lease_inspect`, `sys_audit_drain`
- `sys_debug_write` updated: directly writes a user byte to UART MMIO
- `FRAME_ALLOC` static: `FrameAllocator` moved from kmain stack to BSS static
- `KERNEL_ROOT_PFN` static: kernel root page-table PFN stored for trap-time
  `sys_task_spawn` access
- `FA_RAW_PTR` static: frame-allocator raw pointer stored for `sys_task_spawn`
- TRAP_SCRATCH expanded to `[usize; 3]`: slot [2] holds user sp temp save
- `Scheduler::suspend_current()` used in `block()` (M3 fix carried forward)
- `TaskTable::next_free_index()` helper

### Fixed
- `first_entry`: changed `in(reg) tf` to `in("a0") tf` — when the compiler
  chose `s3` (x19) as the base register, `ld x19, 19*8(s3)` would overwrite
  the base with `TrapFrame.gpr[19]` (often 0), faulting on the next load
- `FrameAllocator` local-to-kmain: trap handler resets sp to `__stack_top`
  on every entry, overwriting kmain's stack frame including `fa_cell`; fix:
  moved to `static FRAME_ALLOC` in BSS
- `SERVICE_STACK_TOP`: was `0x50000`, linker script produces `0x51000`
  (`__stack_bottom=0x41000 + 64K = 0x51000`); incorrect value caused
  `StorePageFault` on first service function call
- `trap/entry.rs`: `sd sp, 2*8(t6)` saved kernel sp (already loaded by
  `ld sp, 0(t6)`) instead of user sp; fixed by: (a) storing user sp to
  `SCRATCH[2]` before loading kernel sp, (b) restoring sscratch to
  SCRATCH_ADDR, (c) loading user sp back from `SCRATCH[2]` into the
  TrapFrame; without this, all service stacks were silently corrupted after
  their first ecall
- `check_smoke_pass` / `task_label`: updated for M4 single-task model (init
  exit = PASS criterion; task labels: init/configd/cap-broker/auditd/…)
- `task/spawn.rs`: `alloc_frame` returns `Result`, not `Option`; replaced
  `ok_or` with `map_err` in all spawn allocation paths
- `task/spawn.rs`: `next_free_index()` returns `Option`; kept `ok_or` there
- `TaskState`: added `#[derive(PartialEq)]` for state comparison in
  `sys_task_status` and `sys_task_start`
- `BlockReason`, `FaultInfo`: added `#[derive(PartialEq)]` for
  `TaskState::PartialEq` to work transitively

---

## [0.0.3] — 2026-05-11 — M3: IPC and Capability

### Added
- `fjell-abi`: expanded `SyscallNumber` (CapCopy/Mint/Delete/Revoke/Inspect,
  IpcSend/Recv/Call/Reply) and `SysError` (InvalidCap, WrongType, SlotEmpty,
  SlotOccupied, RightsExceed, CapTransferDenied, WouldBlock, QueueFull,
  MsgTooLong, Canceled, NoMemory, AlreadyMapped, NotMapped, InvalidAddress)
- `fjell-cap`: `CapHandle` (generation-tagged), `CapRights` bitmask,
  `CapKind`, `Capability`, `CapSlot`, `CSpace` with copy/mint/delete/revoke/
  inspect; derivation tree; host unit tests
- `fjell-ipc`: synchronous rendezvous `Endpoint` (sendq + recvq),
  `PendingMessage` snapshot, `ReplyEdge` (one-shot reply), `MessageTag`;
  host unit tests for IPC-A/B/C invariants
- `fjell-kernel/src/cap/`: `table.rs` (EndpointTable, CapTable, ReplySlot),
  `syscall.rs` (all M3 cap + IPC syscall handlers)
- `fjell-kernel/src/audit/ring.rs`: M3 audit kinds
  (CapCopy/Mint/Delete/Revoke, IpcSend/Recv/Call/Reply/Denied)
- `fjell-kernel/src/task/user_image.rs`: `USER_TASK_C` (denied task — no
  capability, ipc_call → denied → exit(0))
- `main.rs`: 3-task M3 smoke scenario (client, server, denied) with M3 boot
  messages; per-task satp switching; UART mapped in each user page table
- `Scheduler::suspend_current()`: clears current without dequeuing next task
- M3 kernel log messages: "client: call sent", "server: request received",
  "server: reply sent", "client: reply received", "denied: ipc denied as
  expected", "audit: capability.denied", "audit: ipc.call", "audit: ipc.reply"
- Smoke test marker: `TEST:M3:PASS` (all three tasks exit(0) via IPC flow)
- `cargo xtask qemu-test m3` smoke gate

### Fixed
- `fjell-cap/src/cspace.rs`: `gen` renamed to `slot_gen` (Rust 2024 reserved
  keyword)
- `fjell-ipc/src/endpoint.rs`: removed duplicate `pub use` re-export of
  `IPC_WORDS`/`IPC_CAPS` that caused E0252 in Rust 2024
- `fjell-ipc/src/endpoint.rs`: added `#[derive(Debug)]` to `SendResult` and
  `RecvResult` (required by `unwrap_err` in tests)
- `fjell-cap/src/slot.rs`: added `#[derive(PartialEq)]` to `Capability`
  (required by `assert_eq!` in tests)
- `task/user_image.rs`: removed stray `}` after `USER_TASK_A` slice literal
- `cap/table.rs`: removed unused imports `CSPACE_SLOTS`, `CapHandle`,
  `CapKind`, `CapRights`
- `main.rs (m_mode_setup)`: added PMP entry 0 (NAPOT, RWX, all memory) so
  S-mode can access RAM after `mret` — without this every S-mode fetch faults
- `main.rs (kmain)`: DTB reservation now ignores `AlreadyReserved` (DTB may
  overlap kernel image on QEMU virt)
- `main.rs (kmain)`: identity map extended to `__stack_top` (was only
  `boot_end = BSS+2MiB`; stack is BSS+4MiB, causing store_page_fault on
  first kernel stack write after enabling Sv39)
- `mm/page_table.rs (clone_kernel_half)`: now copies root entry 2
  (VA 0x80000000, kernel code/data/stack) instead of entries 256..511 (which
  are all empty for this identity-mapped kernel)
- `trap/dispatch.rs (schedule_next)`: `write_sscratch` called on every
  schedule to restore the sscratch → TRAP_SCRATCH pointer (the trap entry
  `csrrw t6, sscratch, t6` leaves sscratch = user's t6; without this restore
  the second trap would fault at `ld sp, 0(t6)` with t6=0)
- `cap/syscall.rs (block)`: replaced `sched.on_exit()` (which internally
  dequeues and discards the next task) with `sched.suspend_current()` — fixes
  silent task loss when caller blocks on IPC
- `task/scheduler.rs (on_exit/on_fault/on_yield)`: removed internal
  `choose_next()` calls; callers in `schedule_next` now always call
  `choose_next()` exactly once, preventing double-dequeue and task loss
- `cap/syscall.rs (sys_ipc_recv)`: for `is_call=true`, sender is no longer
  prematurely woken on message delivery (it must wait for explicit `ipc_reply`)
- `cap/syscall.rs (sys_ipc_reply)`: caller state is checked before waking;
  Exited/Faulted callers are silently skipped to prevent zombie resurrection


---

## [v0.0.2 — M2: Memory and Task Isolation]

### Added
- M-mode shim → S-mode kernel handoff; `kmain(hart_id, dtb_pa)` signature
- `arch/riscv64/`: CSR helpers, `satp` write, `sfence.vma`, timer (CLINT), PTE types
- `platform/`: `PlatformInfo` with hardcoded QEMU virt constants; DTB pointer forwarding
- `mm/`: `BootAllocator`, bitmap `FrameAllocator`, `FrameOwner`, `MmError`
- `mm/`: `PhysAddr`, `VirtAddr`, `PhysFrame` address types
- `mm/`: Sv39 `PageTable`, `map_page`, `unmap_page`, `translate`; kernel shared map
- `mm/`: `AddressSpace`, `VmRegion`, `VmPerms`, `VmRegionKind`
- `task/`: `TaskId`, `TaskState`, `FaultInfo`, `TrapFrame`, `KernelContext`, `Task`,
           `TaskTable`, `TaskAccounting`
- `task/scheduler.rs`: fixed-priority round-robin `ReadyQueue`, idle task
- `task/user_image.rs`: two embedded RISC-V user tasks (task-a: yield/exit,
                        task-b: yield/fault)
- `trap/`: assembly trap entry (`stvec` direct mode), Rust dispatch, syscall handler,
           fault containment → `TaskState::Faulted`
- `audit/ring.rs`: fixed-capacity append-only `AuditRing`
- `fjell-abi`: split into `lib.rs`, `syscall.rs`, `error.rs`, `task.rs`
- `fjell-tools`: `qemu.rs`, `smoke.rs` submodules; QEMU exit hint (Ctrl-A X)
- QEMU smoke test: `TEST:M2:PASS` marker

### Fixed (v0.0.1 carry-over)
- `publish = false` added to all crate `Cargo.toml` files
- All documentation and comments are now English only
- `cargo xtask qemu` now prints "Press Ctrl-A then X to exit QEMU" on launch
- `code-model=medany` → `code-model=medium` (LLVM name)
- Linker script supplied via `build.rs` (`CARGO_MANIFEST_DIR`-relative absolute path)
- `default-members` excludes `fjell-kernel`; host and kernel builds are cleanly separated
- `#[no_mangle]` → `#[unsafe(no_mangle)]` (edition 2024)
- `static mut UART` → `UnsafeCell`-based console (edition 2024 `static_mut_refs`)

---

## [v0.0.1 — M1: Bootable Kernel]

### Added
- Cargo workspace: resolver = "2", Rust 2024 edition, 17 crate skeletons
- `link.ld`: kernel image at `0x8000_0000`, 4 MiB stack
- `boot.rs`: `_start` assembly — hart-0 selection, BSS zero-fill, stack pointer, `kmain`
- `uart.rs`: NS16550A UART driver (MMIO `0x1000_0000`), `fmt::Write`, CRLF on `\n`
- `console.rs`: `print!` / `println!` macros backed by `UnsafeCell<Uart>`
- `main.rs`: `kmain()` boot banner; panic handler writes to UART then spins
- `build.rs`: linker script via `cargo:rustc-link-arg` with absolute path
- `.cargo/config.toml`: `code-model=medium` for RISC-V target; `xtask` alias
- `crates/fjell-kernel/.cargo/config.toml`: default RISC-V target, QEMU runner
- `.github/workflows/ci.yml`: host check/test + kernel check/build + QEMU M1 smoke test
- `docs/`: full mdBook skeleton (design-philosophy, architecture, ADRs 0001–0005, …)
- `LICENSE` (Apache-2.0), `NOTICE`, `TERMS_OF_USE.md`, `README.md`, `ROADMAP.md`

---

*Releases are tagged once each milestone passes its acceptance criteria.*
