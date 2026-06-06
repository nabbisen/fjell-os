# RFC-v0.10-007 — v1.0 Readiness Matrix

**Status:** Implemented (v0.10.0)
**Target version:** v0.10.0
**Parent:** RFC 061 §10 (roadmap).
**Cross-refs:** all v0.10–v0.15 RFCs.

## 1. Purpose

v1.0 is a discipline release, not an architecture release. Reaching it
requires nothing new to be invented; it requires a list of named
properties to be true at once. v0.10 publishes that list so subsequent
milestones know exactly what they are buying down.

The matrix is a single document checked into the repo. Every cell has
a status: **DONE**, **IN PROGRESS** (with target milestone), **DEFERRED**
(with rationale), or **OPEN** (no plan yet).

## 2. The matrix

Maintained at `docs/release/v1-readiness.md`. v0.10 lands the initial
fill; later milestones update cells they retire.

### 2.1 Identity dimension

| Item | Source | v0.10 status |
|------|--------|--------------|
| Identity statement adopted | RFC 061 §2 | IN PROGRESS (v0.10) |
| Archetypes A1, A2, A3 defined | RFC 061 §3 | IN PROGRESS (v0.10) |
| Non-goals explicitly listed | RFC 061 §3.4, §7 | IN PROGRESS (v0.10) |
| Identity guide published | RFC-v0.10-006 | IN PROGRESS (v0.10) |

### 2.2 Surface dimension

| Item | Source | v0.10 status |
|------|--------|--------------|
| Stable surface enumerated | RFC-v0.10-002 §2 | IN PROGRESS (v0.10) |
| Stability tiers per item | RFC-v0.10-002 §3 | IN PROGRESS (v0.10) |
| `ci-abi-check` gate live | RFC-v0.10-002 §6 | IN PROGRESS (v0.10) |
| ABI snapshot committed | RFC-v0.10-002 | IN PROGRESS (v0.10) |
| SDK_API_REV bound to surface | RFC v0.9-001 | DONE |

### 2.3 Trust spine dimension

| Item | Source | v0.10 status |
|------|--------|--------------|
| HardwareTrustProvider interface | RFC v0.3-001 | DONE |
| Keyring & epoch model | RFC v0.3-002 | DONE |
| Real signature backend (Ed25519) | (v0.11) | IN PROGRESS (v0.11) |
| Bundle signing pipeline | (v0.11) | IN PROGRESS (v0.11) |
| Key revocation records | (v0.11) | IN PROGRESS (v0.11) |
| Replay cache for attestation | (v0.11) | IN PROGRESS (v0.11) |
| Anti-rollback metadata | RFC v0.3-003 | DONE |
| Local attestation profile v2 | RFC v0.3-004 | DONE |

### 2.4 Quality dimension

| Item | Source | v0.10 status |
|------|--------|--------------|
| Host test suite (>480 tests) | n/a | DONE |
| Proptest harness (≥10 properties) | RFC v0.6-001 | DONE |
| Fuzz targets (≥4) | RFC v0.6-003 | DONE |
| Unsafe-audit gate, zero gaps | RFC v0.6-004, RFC 060 | DONE |
| QEMU smoke tier (≥4 profiles) | n/a | DONE |
| QEMU negative tier (≥9 categories) | RFC v0.7.1-002 | DONE |
| Reproducible-build gate | RFC-v0.10-003 | IN PROGRESS (v0.10) |
| Benchmark baseline + regression | RFC-v0.10-004 | IN PROGRESS (v0.10) |

### 2.5 Operability dimension

| Item | Source | v0.10 status |
|------|--------|--------------|
| Reference QEMU fleet demo | RFC-v0.10-005 | IN PROGRESS (v0.10) |
| Trust Report (six sections) | RFC 061 §6 | IN PROGRESS (v0.10) |
| Recovery playbook (key compromise) | (v0.13) | OPEN |
| Bulk re-attestation workflow | (v0.13) | OPEN |
| Staged rollout failure handling | (v0.13) | OPEN |
| Disaster recovery patterns | (v0.13) | OPEN |

### 2.6 Reach dimension

| Item | Source | v0.10 status |
|------|--------|--------------|
| QEMU `virt` profile supported | n/a | DONE |
| First real RISC-V board profile | (v0.12) | IN PROGRESS (v0.12) |
| ARM64 second-platform profile | (v0.12+) | DEFERRED past v1.0 (RFC 061 P1) |
| Hardware bring-up notes | (v0.12) | IN PROGRESS (v0.12) |

### 2.7 Ecosystem dimension

| Item | Source | v0.10 status |
|------|--------|--------------|
| `fjell-sdk` published | RFC v0.9-001 | DONE |
| CapManifest format | RFC v0.9-002 | DONE |
| Bundle format | RFC v0.9-004 | DONE |
| dev-harness | RFC v0.9-005 | DONE |
| First external service | (v0.14) | IN PROGRESS (v0.14) |
| Bundle publishing flow | (v0.14) | IN PROGRESS (v0.14) |
| Typed catalog structs | (v0.14) | IN PROGRESS (v0.14) |

### 2.8 Governance and process dimension

| Item | Source | v0.10 status |
|------|--------|--------------|
| RFC lifecycle policy | RFC 000 | DONE |
| Unsafe charter | RFC v0.6-004 | DONE |
| Release checklist | (v0.15) | IN PROGRESS (v0.15) |
| Security advisory process | (v0.15) | IN PROGRESS (v0.15) |
| LTS branch policy | (post-v1.0) | DEFERRED |
| Contributor invitation policy | (post-v1.0) | DEFERRED |

## 3. Update protocol

When a milestone closes:

1. Each RFC moved to `done/` updates its cells in the matrix.
2. The matrix becomes part of the release tag's evidence (linked
   from CHANGELOG).
3. **OPEN** cells block v1.0 unless explicitly reclassified
   **DEFERRED** by a follow-up RFC.

## 4. Acceptance criteria

1. `docs/release/v1-readiness.md` exists with the matrix as published
   here.
2. Every IN PROGRESS cell names a target milestone.
3. Every DEFERRED cell names the RFC that records the rationale.
4. The matrix is generated (or at least linted) by an xtask so that
   stale cells fail CI.
5. The README of the release ships the matrix snapshot for the tag.

## 5. Out of scope

- Estimating dates for the milestones.
- Tracking individual contributor assignments.
- Forecasting v2.0 work.
