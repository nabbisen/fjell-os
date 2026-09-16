# Fjell OS v0.2 Development Summary

**Milestone:** Security Boundary Closure  
**Versions:** v0.2.0 (2026-05-17) → v0.2.8 (2026-05-18)  
**Gate token:** `TEST:V02:PASS`

---

## What was built

v0.2 addressed a specific problem: the v0.1.0 enforcement audits (RFC 029,
030, 044) showed that many kernel syscalls performed no capability check at
all, or checked only the kind without verifying rights or lease state.  Every
phase of v0.2 closed one category of gap.

### Security primitives added

**`require_cap()` (RFC 031)** — a 7-step capability validation function in
`fjell-cap` that replaces the v0.1 slot-scan approach: CSpace lookup,
generation check, state check, kind check, rights check, scope check, lease
epoch check.  All kernel syscalls that touch protected resources now go
through this path.

**`cap_drop` (RFC 032)** — `sys_cap_drop` succeeds even on revoked
capabilities, allowing CSpace slot reclamation.  Unlike `cap_delete` it
skips the lease check.

**O(1) lease epoch revocation (RFC 033)** — `lt.revoke(id)` increments an
epoch counter rather than scanning capabilities.  Every subsequent use of a
cap bound to the old epoch fails lazily.  `revoke_owned_by(task)` handles
task lifecycle.

**Blocked IPC revocation (RFC 034)** — `cancel_blocked_ipc_for_lease` walks
endpoint recvqs, sendqs, and reply edges.  Blocked callers and receivers are
woken with `SysError::LeaseRevoked` the instant a lease is revoked.

**`sys_cap_bind_lease` (RFC 042 infrastructure)** — new kernel primitive that
allows a user-space service holding a `LeaseAdmin` cap to bind a lease to any
cap in its CSpace, enabling the IPC revocation tests.

### Enforcement boundary closures

| Syscall | v0.1 state | v0.2 state |
|---------|-----------|-----------|
| `sys_mmio_map` | kind check only | `require_cap(MmioRegion, MMIO_MAP)` + RAM guard |
| `sys_dma_alloc` | kind check only | `require_cap(DmaRegion, DMA_ALLOC)` |
| `sys_dma_revoke` | not implemented | Active→Zeroized→Freed state machine |
| `sys_audit_drain` | `RECV` right (wrong bit) | `AUDIT_DRAIN` right |
| `sys_ipc_recv` | no lease check | `RecvWaiter` carries lease binding |
| `sys_ipc_call` | no lease check | `PendingMessage` + `ReplyEdge` carry lease |
| `sys_ipc_reply` | no lease check | late-reply rejected when edge cancelled |
| `sys_task_spawn/start` | ALL rights only | `TASK_CREATE` / `TASK_START` specific rights |
| cap-broker policy | immediate enforcement | `Bootstrap→Enforcing` typestate, `DelegationRecord` |

### New user-facing syscalls

- `sys_cap_drop` (nr 15)
- `sys_cap_bind_lease` (nr 16)
- `sys_dma_revoke` (nr 112)
- `sys_cap_copy`, `sys_cap_mint` — user-space wrappers added
- `sys_ipc_call_words` — passes data words via a2-a4 (cap-broker protocol)

### New format types

- `AuditPersistRecord` (40 bytes) + `AuditLogHeader` (32 bytes) — RFC 041
- `EvidenceRow` + `security_audit_state_node` — RFC 041
- `SnapshotDigest::audit_last_seq` + `audit_dropped_count` — RFC 041
- `EvidenceGapError` + `verify_evidence_continuity` — RFC 041
- `SvcLifecycle`, `ServiceManifestEntry` — RFC 038

---

## Test infrastructure built

### `fjell-neg-test` service

A dedicated user-space service (ImageId 20) that exercises every negative-test
scenario and emits `NEG:*:PASS` markers on the serial port.  Runs as the last
service during boot.

Bootstrap CSpace layout:

| Slot | Cap | Purpose |
|------|-----|---------|
| 0 | Endpoint (object 0) | Own endpoint / IPC with sample-service |
| 1 | AuditDrain | User-copy null/kernel-addr tests |
| 2 | DmaRegion | DMA revoke + zeroize tests |
| 3 | Endpoint (object 5) | cap-broker endpoint for policy tests |
| 4 | LeaseAdmin | Lease creation + binding |
| 5 | TaskCreate | SVC test service spawning |
| 6 | TaskControl | SVC task status monitoring |
| 31-35 | MmioRegion 0-4 | MMIO bounds + RAM guard tests |

### Two-party IPC test protocol (v0.2.5–0.2.6)

The blocked-recv, blocked-call, and late-reply IPC tests required two
cooperating tasks.  Rather than introducing a purpose-built IPC helper
service, `fjell-sample-service` was extended with two new protocol tags:

- `BIND_LEASE_FOR_IPC_TEST (0x060)` — sample-service binds a lease, blocks in
  `ipc_recv`.  When neg-test revokes the lease, the cooperative scheduler
  guarantees sample-service is already in the recvq before neg-test runs.

- `BIND_LEASE_AND_CALL_BACK (0x061)` — sample-service binds a lease to a copy
  of its endpoint cap and calls neg-test back.  One protocol exchange yields
  both `BLOCKED_CALL` (sample-service woken by cancel) and `LATE_REPLY`
  (neg-test's `ipc_reply` on the cancelled edge fails).

### Service crates added

| Crate | Purpose |
|-------|---------|
| `fjell-neg-test` | Negative test runner |
| `fjell-svc-timeout` | Never sends READY (start-timeout test) |
| `fjell-svc-fault` | Yields then reads from NULL (fault test) |

---

## Key design decisions

**`sys_cap_bind_lease` vs cap delegation** — The cleanest way to create
lease-bound caps for IPC revocation tests was to add a `sys_cap_bind_lease`
syscall (requires `LeaseAdmin + LEASE_CREATE`) rather than implement cap
delegation from cap-broker.  Cap delegation (broker installs a cap into
another task's CSpace) is deferred to v0.3.

**Cooperative scheduling as a test guarantee** — Several tests (blocked-recv,
DMA zeroize) rely on the single-hart cooperative scheduler.  After sample-
service calls `sys_ipc_reply`, it stays on-CPU and immediately calls
`sys_ipc_recv` — by the time neg-test runs, sample-service is already in the
recvq.  This is documented in the test comments and is a valid property of the
single-hart M-phase design.

**RAM-guard test region (region 4)** — The existing MMIO regions (max PA
`0x1000_FFFF`) cannot reach RAM at `0x8000_0000`.  A synthetic `neg-test-RAM`
region (`base=0x7FFE_0000, size=0x30000`) was added to the static MMIO table
solely to make the RFC 005 RAM-guard observable in a negative test.

**DMA zeroize via explicit revoke** — `NEG:DMA:ZEROIZE_ON_EXIT:PASS` uses the
explicit-revoke path rather than task exit, because neg-test cannot verify its
own exit.  The two paths share the same `write_bytes(pa, 0, 4096)` kernel call.

---

## Accepted limitations (V02-A-xxx)

| ID | Detail |
|----|--------|
| V02-A-001 | Task/lease syscalls use CSpace slot-scan; handle-based `require_cap` requires ABI changes (v0.3) |
| V02-A-002 | DMA quarantine timeout is synchronous zeroize; timer-callback path deferred |
| V02-A-003 | Service extraction (storaged/bootctl) incomplete; RFC 038 READY types defined but not wired |
| V02-A-004 | cap-broker bootstrap uses trust-the-kernel delivery, not badge-verified sender |
| V02-A-005 | Owner-scope MMIO enforcement deferred to v0.3 cap delegation |

---

## Host test count at close

| Crate | Tests |
|-------|-------|
| `fjell-cap` | 16 |
| `fjell-ipc` | 10 |
| `fjell-tools` (policy_eval) | 14 |
| `fjell-audit-format` | 5 |
| `fjell-snapshot-format` | 5 |
| `fjell-semantic-format` | 4 |
| `fjell-store-format` | 5 |
| `fjell-upgrade-format` | 5 |
| Other formats | ~22 |
| **Total** | **~86** |

All passing.  Zero warnings in both `cargo check` (RISC-V target) and
`cargo test` (host target) at the v0.2.8 close.
