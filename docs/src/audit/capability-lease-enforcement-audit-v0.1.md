# Fjell OS v0.1 — Capability / Lease Enforcement Audit

**Version:** v0.1.3.  
**Produced by:** RFC 029 (also known as RFC-v0.1.x-006).  
**Input contract to:** v0.2 RFC set (primarily RFC 031, 033, 034).

This document classifies every authority-bearing syscall's enforcement
state at v0.1.0 / v0.1.2.

---

## Enforcement Classification

| Label | Meaning |
|---|---|
| `OK` | kind + rights + lease + scope are all checked |
| `Partial` | some checks exist but not all (most common at v0.1.x) |
| `Missing` | no meaningful capability check |
| `DebugOnly` | intentionally debug-gated |
| `Deferred` | not yet implemented |

---

## 1. Capability Management Syscalls

| Syscall | Nr | Kind check | Rights check | Lease check | Scope check | Classification | v0.2 RFC |
|---|---|---|---|---|---|---|---|
| `cap_copy` | 10 | ✓ (type match) | ✗ (no COPY check) | ✗ | ✗ | Partial | RFC 031 |
| `cap_mint` | 11 | ✓ | ✗ (no MINT; rights narrowing not enforced) | ✗ | ✗ | Partial | RFC 031 |
| `cap_delete` | 12 | ✓ (ownership) | ✗ | ✗ (lease revoked cap can't be deleted in some paths) | ✗ | Partial | RFC 032 |
| `cap_revoke` | 13 | ✓ | ✗ (no REVOKE check) | ✗ | ✗ | Partial | RFC 031 |
| `cap_inspect` | 14 | ✓ (ownership) | ✗ (no INSPECT check) | ✗ | ✗ | Partial | RFC 031 |

### Specific gaps

- `cap_mint` does not enforce `child_rights ⊆ parent_rights`. A task
  can mint a capability with *more* rights than the source.
- `cap_delete` does not allow deleting a capability whose lease is
  revoked in all code paths. RFC 032 makes this explicitly allowed.
- No syscall in this group checks a lease binding.

---

## 2. IPC Syscalls

| Syscall | Nr | Kind check | Rights check | Lease check | Scope check | Classification | v0.2 RFC |
|---|---|---|---|---|---|---|---|
| `ipc_send` | 20 | ✓ (Endpoint) | ✗ (no SEND) | ✗ | ✗ | Partial | RFC 031 |
| `ipc_recv` | 21 | ✓ (Endpoint) | ✗ (no RECV) | ✗ | ✗ | Partial | RFC 031 |
| `ipc_call` | 22 | ✓ (Endpoint) | ✗ (no CALL) | ✗ | ✗ | Partial | RFC 031 |
| `ipc_reply` | 23 | ✓ (Reply cap) | ✗ (no REPLY) | ✗ | ✗ | Partial | RFC 031 |
| `ipc_try_recv` | 24 | ✓ (Endpoint) | ✗ (no RECV) | ✗ | ✗ | Partial | RFC 031 |

### Specific gaps

- No IPC syscall checks the `SEND`, `RECV`, `CALL`, or `REPLY` rights bit.
- No IPC syscall checks the lease binding of the endpoint capability.
- A task with any Endpoint cap can send/receive/call/reply regardless
  of which rights that cap carries.
- Blocked IPC has no wake/cancel mechanism on lease revoke (RFC 034).

---

## 3. Task Management Syscalls

| Syscall | Nr | Kind check | Rights check | Lease check | Scope check | Classification | v0.2 RFC |
|---|---|---|---|---|---|---|---|
| `task_spawn` | 40 | ✓ (TaskCreate) | ✗ (no TASK_CREATE) | ✗ | ✗ | Partial | RFC 031 |
| `task_start` | 41 | ✓ (TaskControl) | ✗ (no TASK_START) | ✗ | ✗ (no task-scope) | Partial | RFC 031 |
| `task_status` | 42 | ✓ (TaskInspect) | ✗ (no TASK_STATUS) | ✗ | ✗ | Partial | RFC 031 |
| `task_kill` | 43 | ✓ (TaskControl) | ✗ (no TASK_KILL) | ✗ | ✗ | Partial | RFC 031 |

---

## 4. Lease Management Syscalls

| Syscall | Nr | Kind check | Rights check | Lease check | Scope check | Classification | v0.2 RFC |
|---|---|---|---|---|---|---|---|
| `lease_create` | 50 | ✓ (LeaseAdmin) | ✗ (no LEASE_CREATE) | ✗ | ✗ | Partial | RFC 031 |
| `lease_revoke` | 51 | ✓ (LeaseAdmin) | ✗ (no LEASE_REVOKE) | ✗ | ✗ | Partial | RFC 031 + 033 |
| `lease_inspect` | 52 | ✓ (LeaseAdmin) | ✗ (no LEASE_INSPECT) | ✗ | ✗ | Partial | RFC 031 |

### Specific gap

`lease_revoke` increments epoch in the ABI type but the epoch change
is **not propagated back** to existing capability holders. A revoked
lease should cause all bound capabilities to fail their next use, but
this link is missing until RFC 033.

---

## 5. Audit Syscall

| Syscall | Nr | Kind check | Rights check | Lease check | Scope check | Classification | v0.2 RFC |
|---|---|---|---|---|---|---|---|
| `audit_drain` | 60 | ✗ (no check) | ✗ | ✗ | ✗ | Missing | RFC 039 |

`audit_drain` currently has **no capability check**. Any task can
drain the kernel audit ring. RFC 039 adds `AuditDrain + AUDIT_DRAIN`
enforcement.

---

## 6. Device / Platform Syscalls

| Syscall | Nr | Kind check | Rights check | Lease check | Scope check | Classification | v0.2 RFC |
|---|---|---|---|---|---|---|---|
| `platform_info_get` | 80 | none (read-only, no cap needed) | n/a | n/a | n/a | OK | — |
| `mmio_map` | 90 | ✗ (no cap at all) | ✗ | ✗ | ✗ (RAM guard only) | Missing | RFC 035 |
| `mmio_unmap` | 91 | ✗ (ownership by VA) | ✗ | ✗ | ✗ | Partial | RFC 035 |
| `irq_bind` | 100 | ✓ (Endpoint) | ✗ | ✗ | ✗ | Partial | RFC 031 |
| `irq_ack` | 101 | ✗ (irq-slot ownership) | ✗ | ✗ | ✗ | Partial | — |
| `dma_alloc` | 110 | ✗ (no cap at all) | ✗ | ✗ | ✗ | Missing | RFC 036 |
| `dma_share` | 111 | ✗ | ✗ | ✗ | ✗ | Missing | RFC 036 |
| `dma_revoke` | 112 | ✗ (per-task ownership) | ✗ | ✗ | ✗ | Partial | RFC 036 |
| `reboot` | 120 | ✓ (type-only: Reboot cap) | ✗ (no REBOOT right) | ✗ | ✗ | Partial | RFC 031 |

---

## 7. Bootstrap / Debug

| Item | Classification | Note |
|---|---|---|
| `BootInfo.cap_task_create` | DebugOnly | Development bootstrap only; not a service-pattern |
| `DebugWrite` (nr 2) | DebugOnly | UART write; no enforcement; removed in production builds |
| Bootstrap caps (`lease=None`, `scope=Any`) | DebugOnly | Valid only for `fjell-init`; must be dropped after `cap-broker` enters Enforcing |

---

## 8. Summary

| Classification | Count | Syscalls |
|---|---|---|
| OK | 1 | `platform_info_get` |
| Partial | 18 | (see table above) |
| Missing | 4 | `audit_drain`, `mmio_map`, `dma_alloc`, `dma_share` |
| DebugOnly | 3 | `DebugWrite`, bootstrap caps, `cap_task_create` |
| Deferred | 0 | — |

All `Partial` and `Missing` entries must be resolved in v0.2.0
(primarily RFC 031 for Partial, RFCs 035/036/039 for the device
Missing entries).

---

## 9. v0.2 Enforcement Checklist

Every item below must be `OK` before the v0.2 release gate (RFC 043)
can be signed:

```
☐ require_cap() implemented and used by every Partial/Missing syscall
☐ rights bits checked independently of kind
☐ lease bindings checked on every use (except cap_drop)
☐ scope checked for task-scoped caps (task_start, task_status, task_kill)
☐ rights amplification impossible through cap_mint
☐ audit_drain requires AuditDrain capability
☐ mmio_map requires MmioRegion capability
☐ dma_alloc requires DmaRegion capability
☐ lease_revoke propagates epoch to blocked IPC (RFC 033 + 034)
☐ bootstrap caps dropped after cap-broker enters Enforcing (RFC 040)
```
