# RFC 028: Syscall, ABI, and protocol inventory

**RFC ID:** 028  
**Also known as:** RFC-v0.1.x-005  
**Status:** Proposed  
**Target version:** v0.1.2  
**Affects:** `docs/src/abi/`

## Problem

v0.2 will modify enforcement behaviour at the syscall and protocol
level.  Before any boundary change, the project must know exactly what
ABI surface exists.

Today the syscall table is described inside the kernel source, the
protocol message tags inside `fjell-service-api`, and the persistent
formats inside individual `*-format` crates.  There is no single
document a reviewer can read to confirm coverage.

## Proposed fix

Produce `docs/src/abi/v0.1-inventory.md` as a structured table-driven
document.

### Syscall inventory

For each syscall:

| Column | Meaning |
|---|---|
| number | numeric syscall id (a7) |
| name | symbolic name |
| input registers | a0..a6 usage |
| output registers | a0..a7 usage |
| required capability | which `CapKind`/`CapRights` is required |
| current enforcement status | one of the status labels below |
| stable / unstable | is the contract frozen? |
| target version for hardening | when full enforcement lands |

### Protocol inventory

For each IPC protocol (service-api tag range):

| Column | Meaning |
|---|---|
| protocol id | message tag |
| protocol name | symbolic name |
| request type | payload shape |
| response type | reply shape |
| producer services | who serves the protocol |
| consumer services | who calls it |
| version | major.minor of the protocol |
| stable / unstable | contract frozen? |

### Persistent format inventory

For each `*-format` crate:

| Column | Meaning |
|---|---|
| format name | e.g. `BundleMetadataV2` |
| crate | `fjell-recovery-format` |
| version | structure version |
| checksum / digest | which digest covers the structure |
| canonical encoding status | canonical / partial / none |
| recovery behaviour | how it behaves under corruption |
| stable / unstable | contract frozen? |

### Semantic schema inventory

For each semantic node:

| Column | Meaning |
|---|---|
| schema | StateKind / EventKind / ActionKind |
| version | schema version |
| node type | enum variant |
| producer | service that emits it |
| consumer | service / proxy that renders it |
| compatibility status | additive / breaking |

### Enforcement status labels

```
enforced            full check at every call site
partially-enforced  some sites checked, some not
not-enforced        documented but not checked
development-only    debug-gated check; not for production
deferred            not implemented yet
```

## Rationale

A single inventory is the only practical way to plan v0.2.  Each row
becomes one work item in the v0.2 backlog (RFC 034).

The status labels are deliberately coarse — fine-grained labels would
become stale before the document is published.

## Impact

- Documentation only.  No code changes.
- Adds `docs/src/abi/v0.1-inventory.md`.
- `docs/src/SUMMARY.md` gets a new section.

## Test plan

- Every syscall in `crates/fjell-syscall/src/lib.rs` appears in the
  syscall inventory.
- Every protocol tag in `crates/fjell-service-api/src/lib.rs` appears in
  the protocol inventory.
- Every `*-format` crate appears in the persistent format inventory.
- Every `*Kind` enum variant in `fjell-semantic-format` appears in the
  semantic schema inventory.
- A short script under `crates/fjell-tools` can verify this coverage
  (deferred; manual cross-check is acceptable for v0.1.2).

## Implementation notes

- No ABI stabilization *promise* yet.  This RFC inventories what
  exists; it does not freeze anything.
- No v1 compatibility guarantee implied.
- The inventory becomes the contract reference for v0.2 RFCs.
