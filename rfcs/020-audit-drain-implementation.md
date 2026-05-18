# RFC 020: Audit drain implementation

**RFC ID:** 020  
**Status:** Accepted (implementation deferred to M8 prerequisite)  
**Affects:** `crates/fjell-kernel/src/trap/syscall.rs`, `crates/fjell-auditd/src/main.rs`

## Problem (H-04)

`sys_audit_drain` returns `a0=0` unconditionally.  auditd receives no real events.
M8's measurement / attestation plane depends on kernel audit evidence.

## Proposed fix

```
sys_audit_drain(a0=buf_va, a1=buf_len, a2=audit_drain_cap_handle)
  -> a0=status, a1=n_records_drained, a2=n_dropped
```

1. Validate AuditDrain capability.
2. Validate `buf_va + buf_len` is within caller's user mapping.
3. Walk the kernel `AUDIT` ring from `drain_cursor` to `head`.
4. `copy_to_user(buf_va, records, min(n_available, capacity))`.
5. Advance `drain_cursor`; return `n_drained`, `n_dropped`.

## Prerequisites

- `copy_to_user` helper (safe write to user VA via current task's page table)
- `CapKind::AuditDrain` (new cap kind)
- `AuditRecord` stable binary representation

## Defer condition

`copy_to_user` requires the task's page table root to be accessible at syscall time.
Implement as M8 prerequisite.
