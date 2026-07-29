# External Design — Audit & Observability

*Subsystem 6 of 9. Anchored to FR-AUD-001…005, NFR-SEC-003/004 and
`fjell-kernel/src/audit/`, `fjell-audit-format`, `fjell-auditd` at v0.21.2.*

## 1. Responsibility

The audit subsystem lets an operator or external tool read current system state
safely (FR-AUD-001) and preserves a tamper-evident, ordered record of security-
relevant events (FR-AUD-003…005, NFR-SEC-003/004). It also supports human-
readable state export (FR-AUD-002). Auditability is a first-class requirement,
not a bolt-on.

## 2. External surface

### Continuous audit API (FR-AUD-001)

Read-only by design. The kernel maintains an audit ring buffer; user space
drains it with `sys_audit_drain(cap_handle, buf_va, buf_len)`, which requires an
`AuditDrain` capability. The syscall returns the count of events dropped due to
ring overflow since the last drain, so consumers can detect loss.

### Audit event kinds (as-built, `fjell-audit-format`)

`AuditKind` covers boot, VM map/fault, task create/switch/exit/fault, syscall,
unknown-syscall, capability copy/mint/delete/revoke/drop, lease-revoked, and IPC
send/recv/call/reply/denied — i.e. every security-relevant kernel transition.
User-space services add their own domain events.

### Append-only persistence (FR-SVC-004, as-built)

`AuditPersistRecord` + `AuditLogHeader` define the on-store append-only format;
`auditd` collects kernel-drained events and persists them. The store is
tamper-evident and preserves temporal ordering.

### Plain-text export (FR-AUD-002)

State export targets human-readable forms (TOML / JSON Lines / CSV / Markdown
summary per the requirement). `diagnosticsd` builds diagnostic bundles for this
purpose.

## 3. Requirements coverage

| Requirement | Design response | As-built evidence |
|---|---|---|
| FR-AUD-001 Continuous audit API | Read-only `sys_audit_drain`; cap-gated | `trap/syscall.rs` `sys_audit_drain` |
| FR-AUD-002 Plain-text export | Human-readable export; diagnostic bundles | `diagnosticsd` |
| FR-AUD-003 Delegation audit | Cap create/mint/restrict/revoke emit events | `AuditKind::Cap*`, `LeaseRevoked` |
| FR-AUD-004 Config-change audit | Config change/validate/apply/fail/rollback traceable | `configd` + audit events |
| FR-AUD-005 Fault-event audit | Crash/restart/IPC-error/denial/device-anomaly standardized | `AuditKind::TaskFault`, `IpcDenied`, … |
| NFR-SEC-003 Tamper detection | Append-only store with header integrity | `AuditLogHeader`, `AuditPersistRecord` |
| NFR-SEC-004 Auditability | Security events traceable after the fact | append-only persistence |
| NFR-ACC-004 Human readability | Export formats readable without special tools | plain-text export |

## 4. Design contract: drop-visibility

Because the kernel ring is bounded, overflow is possible under load. Rather than
silently lose events, `sys_audit_drain` reports the drop count since the last
drain. This makes loss observable to the consumer — an availability/honesty
property that matters for audit integrity.

## 5. As-built scope limits & gaps

- The kernel ring is a fixed-capacity buffer (no kernel heap); sustained
  high-rate auditing beyond drain speed will report drops rather than block.
- Full state-export coverage of every FR-AUD-001 field (power state, all device
  state) is present in skeleton form; the exhaustive live inventory is a
  post-v1.0 completeness item.

## 6. Related subsystems

Audit events originate in the [Kernel](./kernel.md) and
[Capability & Lease](./capability-lease.md) subsystems, are persisted by the
[Services](./services.md) plane, and their integrity depends on
[Security & Trust](./security-trust.md).
