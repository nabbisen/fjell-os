# Fjell OS — v1.0 Readiness Matrix

*Governed by RFC-v0.10-007. Every cell must be DONE or DEFERRED
(with rationale) before the v1.0.0 tag. OPEN cells block the release.*

*Last updated: v0.9.4*

---

## Dimension 1 — Identity

| Item | RFC | Status |
|------|-----|--------|
| Identity statement adopted | RFC 061 §2 | **DONE** (v0.9.4) |
| Archetypes A1, A2, A3 defined | RFC 061 §3 | **DONE** (v0.9.4) |
| Non-goals explicitly listed | RFC 061 §3.4, §7 | **DONE** (v0.9.4) |
| Identity guide published (`docs/src/identity/`) | RFC-v0.10-006 | **DONE** (v0.9.4) |

## Dimension 2 — Surface / ABI

| Item | RFC | Status |
|------|-----|--------|
| Stable surface enumerated (S1–S9) | RFC-v0.10-002 §2 | **DONE** (v0.9.4) |
| Stability tiers per item | RFC-v0.10-002 §3 | **DONE** (v0.9.4) |
| `ci-abi-check` gate live | RFC-v0.10-002 §6 | **DONE** (v0.9.4) |
| ABI snapshot committed (`tests/abi/snapshot.json`) | RFC-v0.10-002 | **DONE** (v0.9.4) |
| SDK_API_REV bound to surface | RFC v0.9-001 | **DONE** (v0.9.0) |

## Dimension 3 — Trust Spine

| Item | RFC | Status |
|------|-----|--------|
| HardwareTrustProvider interface | RFC v0.3-001 | **DONE** (v0.3.0) |
| Keyring and KeyEpoch model | RFC v0.3-002 | **DONE** (v0.3.0) |
| Anti-rollback metadata | RFC v0.3-003 | **DONE** (v0.3.0) |
| Attestation profile v2 | RFC v0.3-004 | **DONE** (v0.3.0) |
| Real Ed25519 signature backend | RFC-v0.11-002 | **IN PROGRESS** → v0.11 |
| Bundle signing pipeline | RFC-v0.11-003 | **IN PROGRESS** → v0.11 |
| Key rotation and revocation records | RFC-v0.11-004 | **IN PROGRESS** → v0.11 |
| Replay cache and attestation freshness | RFC-v0.11-005 | **IN PROGRESS** → v0.11 |

## Dimension 4 — Quality / Verification

| Item | RFC | Status |
|------|-----|--------|
| Host test suite (≥ 487 tests) | — | **DONE** (v0.9.4) |
| Proptest harness (≥ 10 properties) | RFC v0.6-001 | **DONE** (v0.6.0) |
| Fuzz targets (≥ 4) | RFC v0.6-003 | **DONE** (v0.6.0) |
| Unsafe-audit gate, zero gaps | RFC v0.6-004, RFC 060 | **DONE** (v0.8.24) |
| QEMU smoke tier (≥ 4 profiles) | — | **DONE** (v0.8.0) |
| QEMU negative tier (≥ 9 categories) | RFC-v0.7.1-002 | **DONE** (v0.7.4) |
| Reproducible-build gate | RFC-v0.10-003 | **DONE** (v0.9.4) |
| ABI snapshot gate | RFC-v0.10-002 | **DONE** (v0.9.4) |
| Benchmark baseline + regression gate | RFC-v0.10-004 | **IN PROGRESS** → v0.10 |
| MMIO ordering audit | RFC-v0.12-004 | **IN PROGRESS** → v0.12 |

## Dimension 5 — Operability

| Item | RFC | Status |
|------|-----|--------|
| Reference QEMU fleet demo | RFC-v0.10-005 | **IN PROGRESS** → v0.10 |
| Trust Report (six sections) | RFC 061 §6 | **DONE** (v0.9.4) |
| Fleet partition reconciliation | RFC-v0.13-002 | **IN PROGRESS** → v0.13 |
| Key compromise recovery playbook | RFC-v0.13-003 | **IN PROGRESS** → v0.13 |
| Bulk re-attestation workflow | RFC-v0.13-004 | **IN PROGRESS** → v0.13 |
| Staged rollout failure handling | RFC-v0.13-004 | **IN PROGRESS** → v0.13 |
| Disaster recovery patterns | RFC-v0.13-005 | **IN PROGRESS** → v0.13 |

## Dimension 6 — Reach / Deployment

| Item | RFC | Status |
|------|-----|--------|
| QEMU `virt` profile supported | — | **DONE** (v0.1.0) |
| DTB and boot handoff validation | RFC-v0.12-003 | **IN PROGRESS** → v0.12 |
| First real RISC-V board profile | RFC-v0.12-002 | **IN PROGRESS** → v0.12 |
| Field operations deployment guide | RFC-v0.12-005 | **IN PROGRESS** → v0.12 |
| ARM64 second-platform | RFC 061 §P1 | **DEFERRED** — post-v1.0 (RFC 061 §5 P1) |

## Dimension 7 — Ecosystem / SDK

| Item | RFC | Status |
|------|-----|--------|
| `fjell-sdk` published | RFC v0.9-001 | **DONE** (v0.9.0) |
| CapManifest format | RFC v0.9-002 | **DONE** (v0.9.0) |
| Bundle format | RFC v0.9-004 | **DONE** (v0.9.0) |
| Dev-harness | RFC v0.9-005 | **DONE** (v0.9.0) |
| Typed catalog structs + cookbook | RFC-v0.14-003 | **IN PROGRESS** → v0.14 |
| First external service (reference) | RFC-v0.14-002 | **IN PROGRESS** → v0.14 |
| Bundle publishing flow + registry | RFC-v0.14-004 | **IN PROGRESS** → v0.14 |
| Developer mode tooling | RFC-v0.14-005 | **IN PROGRESS** → v0.14 |

## Dimension 8 — Governance and Process

| Item | RFC | Status |
|------|-----|--------|
| RFC lifecycle policy | RFC 000 | **DONE** (v0.1.0) |
| Unsafe charter | RFC v0.6-004 | **DONE** (v0.6.0) |
| Threat model finalized | RFC-v0.15-002 | **IN PROGRESS** → v0.15 |
| Release checklist | RFC-v0.15-003 | **IN PROGRESS** → v0.15 |
| Security advisory process | RFC-v0.15-003 | **IN PROGRESS** → v0.15 |
| Operator recovery guide | RFC-v0.15-004 | **IN PROGRESS** → v0.15 |
| v1.0 non-goals locked | RFC-v0.15-005 | **IN PROGRESS** → v0.15 |
| LTS branch policy | — | **DEFERRED** — post-v1.0 |
| Contributor governance | — | **DEFERRED** — post-v1.0 |

---

## Summary at v0.9.4

| Status | Count |
|--------|-------|
| DONE | 22 |
| IN PROGRESS | 20 |
| DEFERRED | 3 |
| OPEN | 0 |

*No OPEN cells. Every IN PROGRESS item has a target milestone.*

---

*CI gate: `cargo xtask readiness-check` counts OPEN cells and fails
if any are present. Maintained by [`tools/fjell-readiness-check/`](../../tools/fjell-readiness-check/)
(RFC-v0.10-007 §4 — tool lands in v0.10 cycle).*
