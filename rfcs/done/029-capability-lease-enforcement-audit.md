# RFC 029: Capability / lease enforcement audit

**RFC ID:** 029  
**Also known as:** RFC-v0.1.x-006  
**Status:** Implemented (v0.1.3)
**Target version:** v0.1.3  
**Affects:** `docs/src/audit/`, no code

## Problem

Fjell OS depends on capability enforcement for every privileged
operation.  v0.1.x must not silently assume that all paths are
protected.  Without a per-operation audit, v0.2 work would begin from
a guess at what is actually missing.

## Proposed fix

Produce `docs/src/audit/capability-lease-enforcement-audit-v0.1.md`
covering every syscall and IPC entry point that touches a capability
or a lease, and classify each.

### Review areas

```
cap_copy
cap_mint
cap_drop
cap_inspect
ipc_send
ipc_recv
ipc_try_recv
ipc_call
ipc_reply
task_spawn
task_start
task_status
lease_create
lease_revoke
lease_inspect
mmio_map
dma_alloc
audit_drain
boot_evidence_get
reboot
```

### Classification labels

Each operation must be classified as exactly one of:

| Label | Meaning |
|---|---|
| **OK** | kind + rights + lease + scope are all checked |
| **Partial** | some checks exist but not all |
| **Missing** | no meaningful capability check |
| **DebugOnly** | intentionally debug-gated; not for production |
| **Deferred** | not implemented yet |

### Required output

For every operation:

- file:line of the current check site (or "missing")
- existing check description
- gap relative to the v0.2 enforcement target
- v0.2 RFC reference (RFC-014 or later)
- negative-test name from RFC 026

## Rationale

A single classification table is the v0.2 work list.  Anything
labelled **Partial** or **Missing** is, by definition, a v0.2 backlog
item.  Anything labelled **OK** is locked in v0.1.x and becomes a
regression-test target.

The classification is intentionally not a free-text quality grade.
Five buckets force unambiguous decisions.

## Impact

- Documentation only.  No code changes.
- Adds one audit document; referenced from the v0.2 roadmap (RFC 034).

## Test plan

- Audit document exists.
- Every operation in the review-areas list is classified.
- Every **Partial** / **Missing** item has a v0.2 RFC reference.
- Every **Partial** / **Missing** item names a negative test from
  RFC 026.
- v0.2 roadmap (`docs/src/roadmap/v0.2-security-boundary.md`) links to
  the audit.

## Implementation notes

- Not all fixes are required in v0.1.x.  Full closure belongs to v0.2.
- The audit is a *snapshot* of v0.1.0 state.  v0.2 RFCs reference it
  and may add follow-up audits, but this document is frozen at the
  v0.1.3 tag.
