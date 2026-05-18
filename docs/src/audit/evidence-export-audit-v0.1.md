# Fjell OS v0.1 — Audit / Snapshot / Semantic Evidence Export Audit

**Version:** v0.1.3.  
**Produced by:** RFC 044 (also known as RFC-v0.1.x-008).  
**Input contract to:** RFC 041 (Persistent Evidence Hardening, v0.2).

This document maps every security-relevant event to every output
channel and records gaps.

---

## Channel key

| Column | Channel | Notes |
|---|---|---|
| audit | kernel audit ring (`AuditRecord`) | v0.1.2: placeholder; real binary drain lands RFC 039 |
| store | `fjell-storaged` persistent projection | v0.1.2: IPC-persisted JSON via auditd / semantic-stream |
| snapshot | `fjell-snapshotd` snapshot record | SHA-256 over JSON blob |
| semantic | `fjell-semantic-stream` StateNode / EventNode | consumed by `fjell-proxy-text` |

---

## Evidence matrix

| Event / Failure | audit | store | snapshot | semantic | Implemented? | Gap | v0.2 target |
|---|---|---|---|---|---|---|---|
| Capability denied | Partial | No | No (counter) | No | No | Not surfaced to semantic or store | RFC 041 |
| Capability granted | No | No | No | No | No | No channel captures grant | RFC 031 + 041 |
| Capability revoked | No | No | No | No | No | Revoke is advisory; no audit event emitted | RFC 033 + 041 |
| Lease revoked | No | No | No | No | No | No audit event | RFC 033 + 041 |
| IPC denied | No | No | No | No | No | No rejection audit | RFC 031 + 041 |
| MMIO denied | No | No | No | No | No | `mmio_map` has no enforcement | RFC 035 + 041 |
| DMA revoked | Partial | No | No | No | No | No quarantine-state event | RFC 036 + 041 |
| Store corruption detected | Partial (CRC32 rejection) | Yes (recovery log) | Yes (snapshot field) | Yes (StateNode) | Partial | Dropped count not visible; no explicit "corruption" audit record | RFC 041 |
| Store recovery completed | No | Yes (recovery log) | Yes | Yes | Partial | Not emitted to audit ring | RFC 041 |
| Invalid release rejected | Yes (verifyd) | Yes | No | Yes | Partial | snapshot does not record rejection | RFC 041 |
| Invalid policy rejected | No | No | No | No | No | cap-broker has no rejection audit | RFC 040 + 041 |
| Rootfs digest mismatch | Yes (verifyd) | Yes | Yes | Yes | Yes | — | — |
| Health target failed | Yes | Yes | Yes | Yes | Yes | — | — |
| Rollback selected | Yes | Yes | Yes | Yes | Yes | — | — |
| Snapshot created | No | Yes (meta) | n/a | Yes | Partial | Not written to audit ring | RFC 041 |
| Attestation generated | No | Yes | No | Yes | Partial | Not in audit ring; not in snapshot | RFC 041 |
| Stale bundle rejected | Yes (verifyd) | Yes | No | Yes | Partial | Not in snapshot | RFC 041 |

### Additional items

| Item | Status |
|---|---|
| Audit ring overflow / dropped count visible | Not implemented — no count field exposed to user space |
| Audit ring drain is real binary records | Not implemented — placeholder at v0.1.x |
| Audit projection persists through storaged | Not implemented — auditd uses in-memory buffer only |
| Capability denial rate in snapshot | Not implemented |
| DMA quarantine timeout in semantic state | Not implemented |

---

## Gaps requiring v0.2 fixes (RFC 041)

### Critical gaps (every channel missing)

1. **Capability denied** — no audit, no store, no snapshot, no semantic.
2. **Capability granted** — no channel captures successful grants.
3. **Capability revoked** — revoke is advisory; no signal exits the kernel.
4. **Lease revoked** — no audit event.
5. **IPC denied** — no channel.
6. **MMIO denied** — enforcement doesn't exist yet (RFC 035 prerequisite).
7. **Invalid policy rejected** — cap-broker has no rejection path (RFC 040 prerequisite).

### Partial gaps (some channels present)

8. **Store corruption** — missing: dropped count; explicit audit record.
9. **Store recovery** — missing: audit ring record.
10. **Invalid release rejected** — missing: snapshot record.
11. **Snapshot created** — missing: audit record.
12. **Attestation generated** — missing: audit, snapshot.
13. **Stale bundle rejected** — missing: snapshot.

### Infrastructure gap

14. **Audit ring drain** — binary `AuditRecord` + real drain +
    dropped-count reporting needed before any of the above can route
    through the audit channel (RFC 039 prerequisite).

---

## Evidence visibility matrix (post-v0.2 target)

This is the **target state** after RFC 041 closes the gaps:

| Event | audit | store | snapshot | semantic |
|---|---|---|---|---|
| Capability denied | ✓ | ✓ | (counter) | ✓ |
| Capability granted | ✓ | ✓ | — | ✓ |
| Capability revoked | ✓ | ✓ | — | ✓ |
| Lease revoked | ✓ | ✓ | — | ✓ |
| IPC denied | ✓ | ✓ | — | ✓ |
| MMIO denied | ✓ | ✓ | — | ✓ |
| DMA revoked / quarantined | ✓ | ✓ | (counter) | ✓ |
| Store corruption | ✓ | ✓ | ✓ | ✓ |
| Store recovery | ✓ | ✓ | ✓ | ✓ |
| Invalid release rejected | ✓ | ✓ | ✓ | ✓ |
| Invalid policy rejected | ✓ | ✓ | — | ✓ |
| Rootfs digest mismatch | ✓ | ✓ | ✓ | ✓ |
| Health target failed | ✓ | ✓ | ✓ | ✓ |
| Rollback selected | ✓ | ✓ | ✓ | ✓ |
| Snapshot created | ✓ | ✓ | — | ✓ |
| Attestation generated | ✓ | ✓ | — | ✓ |
| Stale bundle rejected | ✓ | ✓ | ✓ | ✓ |

This matrix is normative for v0.2. RFC 041 implementations must
satisfy every `✓` cell above.

---

## How to update this document

When a v0.2 RFC closes a gap:

1. Update the "Implemented?" column in the evidence matrix.
2. Update the "Gap" column to empty or `—`.
3. Update the post-v0.2 target section if the target changed.
4. Add an RFC cross-reference in the "v0.2 target" column.

The target state matrix is locked at v0.2.0 tagging (per RFC 043).
