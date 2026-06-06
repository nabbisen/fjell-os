# Fjell OS v0.7.x Hardening RFC Set

This directory contains the patch RFCs filed in response to two architect
review documents covering v0.7.0:

- `fjell_os_v0_7_0_detailed_review-whole-project.md` (1042 lines)
- `fjell_os_v0_7_0_crates_detailed_review.md` (1410 lines)

The reviews identified **6 release blockers, 8 high-priority findings, and
17 medium-priority findings**.  This RFC set closes all release blockers and
high-priority findings, and addresses the medium-priority items that block
v0.8 entry.

Patch-version order follows the whole-project review's *Recommended Fix
Order* (§10) and the crates review's *v0.7.x Hardening Plan* (§12).

---

## RFC Index

### v0.7.1 — Release engineering and runtime integration

| RFC | Closes | Severity |
|-----|--------|----------|
| [RFC-v0.7.1-001](RFC-v0.7.1-001-release-metadata-and-reproducibility.md) | W-RB-06 | Release Blocker |
| [RFC-v0.7.1-002](RFC-v0.7.1-002-ci-coverage-and-verification-gates.md) | W-RB-02, W-M-05, W-M-06, W-M-07 | Release Blocker |
| [RFC-v0.7.1-003](RFC-v0.7.1-003-service-plane-integration.md) | W-RB-01, W-M-04 | Release Blocker |

### v0.7.2 — Distributed sync activation

| RFC | Closes | Severity |
|-----|--------|----------|
| [RFC-v0.7.2-001](RFC-v0.7.2-001-distributed-sync-service-wiring.md) | W-RB-03, C-H-07 | Release Blocker |
| [RFC-v0.7.2-002](RFC-v0.7.2-002-snapshot-envelope-safety-and-merge-rules.md) | C-RB-02, C-M-01, C-M-02, C-H-08, W-H-03 | Release Blocker + High |
| [RFC-v0.7.2-003](RFC-v0.7.2-003-node-identity-safety-and-trust-mode-fail-closed.md) | C-H-02, C-H-03, C-H-04, W-H-04, C-M-05, C-M-10 | High |

### v0.7.3 — Networking and crypto

| RFC | Closes | Severity |
|-----|--------|----------|
| [RFC-v0.7.3-001](RFC-v0.7.3-001-networking-runtime-completion.md) | W-RB-04, C-M-07 | Release Blocker |
| [RFC-v0.7.3-002](RFC-v0.7.3-002-crypto-profile-documentation.md) | C-H-01 | High |

### v0.7.4 — Kernel boundary hardening

| RFC | Closes | Severity |
|-----|--------|----------|
| [RFC-v0.7.4-001](RFC-v0.7.4-001-dma-lifetime-safety.md) | C-RB-01, W-RB-05 | Release Blocker (CRITICAL) |
| [RFC-v0.7.4-002](RFC-v0.7.4-002-mmio-mapping-failure-and-user-copy-docs.md) | W-H-01, C-H-05, W-H-07, C-M-08 | High |
| [RFC-v0.7.4-003](RFC-v0.7.4-003-capability-authority-hardening.md) | C-RB-03, C-RB-04, C-RB-05, C-H-06, C-M-09, W-H-02, W-H-06 | Release Blocker + High |

### v0.7.5 — Catalog metadata and documentation currency

| RFC | Closes | Severity |
|-----|--------|----------|
| [RFC-v0.7.5-001](RFC-v0.7.5-001-catalog-ownership-and-documentation-currency.md) | W-M-01, W-M-02, W-M-03, C-M-03, C-M-04, C-M-06, W-H-05 | Medium + High |

---

## Mapping from architect findings to RFCs

**Notation:** `W-` = whole-project review, `C-` = crates review.

### Release Blockers

| Finding | Description | Closed by |
|---------|-------------|-----------|
| W-RB-01 | v0.4-v0.7 services not in QEMU/ImageId tables | RFC-v0.7.1-003 |
| W-RB-02 | CI does not cover 41/67 packages | RFC-v0.7.1-002 |
| W-RB-03 | identityd/summaryd/syncd are stubs | RFC-v0.7.2-001 |
| W-RB-04 | v0.4 networking still alpha-stub | RFC-v0.7.3-001 |
| W-RB-05 | DMA boundary tracking incomplete | RFC-v0.7.4-001 |
| W-RB-06 | Release metadata stale, no toolchain pin | RFC-v0.7.1-001 |
| C-RB-01 | DMA revoke frees frame while user PTE live | RFC-v0.7.4-001 (CRITICAL) |
| C-RB-02 | snapshot_digest buffer overflow at capacity | RFC-v0.7.2-002 |
| C-RB-03 | Broad unleased MMIO caps to all services | RFC-v0.7.4-003 |
| C-RB-04 | sys_cap_install unrestricted unleased caps | RFC-v0.7.4-003 |
| C-RB-05 | sys_cap_bind_lease no LeaseAdmin check | RFC-v0.7.4-003 |

### High Priority

| Finding | Description | Closed by |
|---------|-------------|-----------|
| W-H-01 / C-H-05 | sys_mmio_map ignores remap failures, overflow | RFC-v0.7.4-002 |
| W-H-02 | Blocked IPC cancellation not unified | RFC-v0.7.4-003 |
| W-H-03 | Snapshot signing domain needs cipher metadata | RFC-v0.7.2-002 |
| W-H-04 | TrustMode::Fleet/Open need fail-closed | RFC-v0.7.2-003 |
| W-H-05 | Unsafe audit too shallow (comment-presence only) | RFC-v0.7.5-001 |
| W-H-06 | Provider replace/remove needs Enforcing policy | RFC-v0.7.4-003 |
| W-H-07 / C-M-08 | copy_to_user docs overclaim VMA validation | RFC-v0.7.4-002 |
| C-H-01 | fjell-sxt-crypto not production-grade | RFC-v0.7.3-002 |
| C-H-02 | NodeIdentityPolicy::permits() can panic | RFC-v0.7.2-003 |
| C-H-03 | Fleet only checks roster_ref exists | RFC-v0.7.2-003 |
| C-H-04 | NodeIdentity::new() creates zero-digest | RFC-v0.7.2-003 |
| C-H-06 | CapRights duplicated in cap-broker | RFC-v0.7.4-003 |
| C-H-07 | rootfsd/snapshotd/virtio-blk also stubs | RFC-v0.7.2-001 |
| C-H-08 | Snapshot v2 format without merge rules | RFC-v0.7.2-002 |

### Medium Priority

| Finding | Description | Closed by |
|---------|-------------|-----------|
| W-M-01 | README & comments contain alpha-era refs | RFC-v0.7.5-001 |
| W-M-02 | Semantic catalog needs ownership metadata | RFC-v0.7.5-001 |
| W-M-03 / C-M-03 | Legacy SnapshotDigest placeholders | RFC-v0.7.5-001 |
| W-M-04 | QEMU xtask still M1-M8 only | RFC-v0.7.1-003 |
| W-M-05 | ARM64 stub has no CI matrix | RFC-v0.7.1-002 |
| W-M-06 | Fuzz targets not integrated in CI | RFC-v0.7.1-002 |
| W-M-07 | default-members=[] hides coverage gaps | RFC-v0.7.1-002 |
| C-M-01 | ConflictDomain::Default conflicts with v1 spec | RFC-v0.7.2-002 |
| C-M-02 | SnapshotRecord push no body_len validation | RFC-v0.7.2-002 |
| C-M-04 | Summaries allow duplicate entries | RFC-v0.7.5-001 |
| C-M-05 | NodeAlias::as_str() hides invalid UTF-8 | RFC-v0.7.2-003 |
| C-M-06 | sys_platform_info_get exposes raw PA | RFC-v0.7.5-001 |
| C-M-07 | Debug UART writes in IPC send path | RFC-v0.7.3-001 |
| C-M-09 | cap_copy resets parent weakens revoke | RFC-v0.7.4-003 |
| C-M-10 | syncd has no replay cache | RFC-v0.7.2-003 |

---

## Patch version timeline (proposed)

```
v0.7.1   Release engineering and runtime integration   (3 RFCs)
v0.7.2   Distributed sync activation                   (3 RFCs)
v0.7.3   Networking and crypto                         (2 RFCs)
v0.7.4   Kernel boundary hardening                     (3 RFCs)
v0.7.5   Catalog metadata and documentation            (1 RFC)
─────────────────────────────────────────────────────────────
         12 RFCs total
```

Each patch version may produce its own release tarball.  v0.8 entry requires
all v0.7.x RFCs accepted and their acceptance tests passing.

---

## Inter-RFC dependencies

```
                                 RFC-v0.7.1-001 (metadata)
                                         │
                       ┌─────────────────┴─────────────────┐
                       │                                   │
              RFC-v0.7.1-002 (CI)                  RFC-v0.7.1-003 (services)
                       │                                   │
                       └─────────────────┬─────────────────┘
                                         │
                       ┌─────────────────┼─────────────────┬─────────────┐
                       │                 │                 │             │
              RFC-v0.7.2-001        RFC-v0.7.2-002    RFC-v0.7.2-003     │
              (sync wiring)          (snapshot       (identity safety)   │
                       │              safety)              │             │
                       │                 │                 │             │
                       │                 │            RFC-v0.7.3-001     │
                       │                 │            (net runtime)      │
                       │                 │                 │             │
                       │                 │            RFC-v0.7.3-002     │
                       │                 │            (crypto profile)   │
                       │                 │                 │             │
              ┌────────┴─────────┬───────┴──────────┐      │             │
              │                  │                  │      │             │
       RFC-v0.7.4-001    RFC-v0.7.4-002    RFC-v0.7.4-003  │             │
         (DMA safety)       (MMIO/copy)      (cap hardening)             │
              │                  │                  │      │             │
              └──────────────────┴──────────────────┴──────┴─────────────┘
                                         │
                                 RFC-v0.7.5-001 (polish)
                                         │
                                       v0.8
```

Hard dependencies:
- v0.7.2-001 (service wiring) depends on v0.7.1-003 (service plane integration)
- v0.7.4-001 (DMA safety) blocks v0.7.3-001 (net runtime) because the
  net driver depends on safe DMA primitives.
- v0.7.5-001 (polish) consolidates documentation and is the final entry
  step before v0.8.
