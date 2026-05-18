# RFC 041: Persistent evidence hardening

**RFC ID:** 041  
**Also known as:** RFC-v0.2-011  
**Status:** Proposed  
**Target version:** v0.2.0  
**Phase:** Phase 8 — Persistent Evidence Hardening  
**Related epics:** E (Persistent Evidence Hardening)

## Problem

After RFC 039 the audit drain produces real records.  After RFCs
031–040 the kernel rejects unauthorised operations.  But evidence
that those rejections happened must reliably reach durable storage
and the semantic stream — otherwise security events are silently
dropped during failure investigation.

RFC 008 (Audit / Snapshot / Semantic Evidence Audit) maps where
events should appear; this RFC closes the gaps it finds.

## Proposed fix

### auditd → storaged persistence

`auditd` persists its converted audit projection through
`storaged`, not through ad-hoc in-memory storage.  Records pass
through the append-only state store with the standard recovery
contract.

### Semantic publication of audit failure states

For each security-sensitive failure category, the semantic stream
gains a state node:

```
[STATE][Failed] Audit drain overflow
[STATE][Failed] Capability denial rate elevated
[STATE][Failed] DMA quarantine timeout
[STATE][Failed] Store corruption detected
[STATE][Failed] Rollback executed
```

### Snapshot inclusion

The system snapshot (M7 schema) extends to record:

```
- audit_last_seq: u64
- audit_dropped_count: u64
- last_recovery_event: Option<RecoveryEvent>
- last_rollback_event: Option<RollbackEvent>
- last_capability_denial_seq: Option<u64>
- last_dma_quarantine_timeout_seq: Option<u64>
```

### Required failure visibility

Every event below must appear in **at least** the listed channels:

| Event | audit | storaged | snapshot | semantic |
|---|---|---|---|---|
| Capability denied | ✓ | ✓ | (counter) | ✓ |
| Capability revoked | ✓ | ✓ | — | ✓ |
| Lease revoked | ✓ | ✓ | — | ✓ |
| IPC denied | ✓ | ✓ | — | ✓ |
| MMIO denied | ✓ | ✓ | — | ✓ |
| DMA revoked / quarantined | ✓ | ✓ | (counter) | ✓ |
| Store corruption | ✓ | ✓ | ✓ | ✓ |
| Store recovery completed | ✓ | ✓ | ✓ | ✓ |
| Invalid release rejected | ✓ | ✓ | — | ✓ |
| Invalid policy rejected | ✓ | ✓ | — | ✓ |
| Rootfs digest mismatch | ✓ | ✓ | ✓ | ✓ |
| Health target failed | ✓ | ✓ | ✓ | ✓ |
| Rollback selected | ✓ | ✓ | ✓ | ✓ |
| Snapshot created | ✓ | ✓ | — | ✓ |
| Attestation generated | ✓ | ✓ | — | ✓ |
| Stale bundle rejected | ✓ | ✓ | — | ✓ |

## Rationale

A single matrix is the only way to keep evidence channels aligned.
Without it, a future "fix" to one channel can silently desynchronise
the others, which is the exact problem RFC 008 calls out.

Routing audit through `storaged` rather than via a parallel store
keeps the recovery contract (RFC 023) applicable to audit data: a
corrupted audit log is detected and recovered the same way every
other persistent stream is.

The snapshot counters (rather than full event copies) are a
deliberate space tradeoff — the audit log is the source of truth
for individual events; the snapshot just reveals "how many".

## Impact

- Crates: `fjell-auditd`, `fjell-storaged`, `fjell-snapshotd`,
  `fjell-semantic-stream`, `fjell-proxy-text` (renderer for new
  state nodes), `fjell-semantic-format` (new node variants if
  needed).
- Backward compatibility: additive in the semantic/snapshot schemas
  (new node kinds, new snapshot fields).

## Test plan

### QEMU negative tests
- `NEG:EVIDENCE:CAP_DENIAL_IN_AUDIT:PASS` — cap denial event present
  in drained audit.
- `NEG:EVIDENCE:ROLLBACK_IN_AUDIT:PASS`
- `NEG:EVIDENCE:DMA_TIMEOUT_IN_SEMANTIC:PASS`
- `NEG:EVIDENCE:STORE_CORRUPTION_IN_SNAPSHOT:PASS`
- `NEG:EVIDENCE:DROPPED_COUNT_VISIBLE:PASS`

### Acceptance gates
- Evidence export does not hide failure.
- Audit, snapshot, and semantic state are aligned for every event
  in the matrix.
- The "dropped count" is visible without crashing the system.

## Implementation notes

- Out of scope: encrypted audit storage, audit retention rotation
  policy (the ring is fixed-capacity per M5), remote audit shipping
  (depends on network — deferred to v0.4+).
- The matrix above is normative for v0.2; new event categories
  added by later RFCs must extend it.
