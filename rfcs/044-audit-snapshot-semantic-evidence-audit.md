# RFC 044: Audit / snapshot / semantic evidence audit

**RFC ID:** 044  
**Also known as:** RFC-v0.1.x-008  
**Status:** Proposed  
**Target version:** v0.1.3  
**Affects:** `docs/src/audit/`, no code

## Problem

Fjell OS must not hide security-relevant failures.  Across the
audit ring, the persistent state store, the semantic stream, the
measurement chain, and local attestation records, every important
event should appear in at least one of these channels.

Today there is no mapping that proves this.  A failure that
silently misses every channel is indistinguishable from no failure
happening at all, which defeats the purpose of evidence collection.

## Proposed fix

Produce `docs/src/audit/evidence-export-audit-v0.1.md` as a single
**matrix** that classifies every event below across every channel.

### Required matrix columns

| Column | Meaning |
|---|---|
| event / failure | category name |
| audit record | Yes / No / Partial; with audit-kind name |
| persistent store record | Yes / No / Partial |
| snapshot field | which snapshot field carries it |
| semantic node | which state/event/intent variant |
| measurement event | applicable kind |
| attestation claim | applicable claim |
| implemented? | Yes / No / Partial |
| gap | description |
| target version | when the gap closes |

### Required events to cover

```
- capability denied
- capability granted
- capability revoked
- lease revoked
- IPC denied
- MMIO denied
- DMA revoked
- store corruption detected
- store recovery completed
- invalid release rejected
- invalid policy rejected
- rootfs digest mismatch
- health target failed
- rollback selected
- snapshot created
- attestation generated
- stale bundle rejected
```

### Required output

For each event:

- which channels currently carry it,
- whether dropped/corrupt counts are exposed,
- gaps that v0.2 (RFC 041) must close,
- gaps that v0.3+ accepts as deferred.

## Rationale

A flat matrix is the only practical way to verify cross-channel
alignment.  Without it, RFC 041 (Persistent Evidence Hardening)
would be designed by guesswork.

The audit lives at the v0.1.x layer because the *measurement* is
v0.1.x work — actual closure happens in v0.2 (RFC 041).  Splitting
audit from fix keeps each PR small and verifiable.

## Impact

- Documentation only.  No code changes.
- References existing RFCs (M5 audit ring, M7 snapshot schema, M8
  measurement chain / attestation record).
- Becomes the input contract to RFC 041.

## Test plan

- Document exists.
- Every event in the required-events list is a row in the matrix.
- Every cell is filled (no blanks).
- Every gap maps to an existing RFC or has an explicit
  "deferred-to-v0.X" annotation.
- README links to the document.

## Implementation notes

- Non-goals: full evidence redesign, remote attestation,
  production audit store.
- The matrix is the *baseline* for v0.2's RFC 041.  When v0.2
  closes a gap, RFC 041 updates the corresponding row's
  "implemented?" column rather than rewriting the whole document.
