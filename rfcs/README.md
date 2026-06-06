# Fjell OS — RFC Index

Folder is the source of truth for state (see
[RFC 000](./done/000-rfc-lifecycle-policy.md)).

```
rfcs/
  proposed/   — open; not yet shipped
  done/       — implemented; historical record
  archive/    — withdrawn or superseded
```

---

## Implemented (done/) — 99 RFCs

### v0.1.0 — M0–M8 prototype (000–025, 048–059)

| ID | Title | Shipped |
|----|-------|---------|
| 000 | [RFC Lifecycle Policy](./done/000-rfc-lifecycle-policy.md) | v0.1.0 |
| 001 | [Fix t5/t6 register save in trap entry](./done/001-fix-trap-entry-t5-t6-register-save.md) | v0.1.0 |
| 002 | [Fix BootControlBlock initial slot B state](./done/002-fix-bootcontrolblock-slot-b-initial-state.md) | v0.1.0 |
| 003 | [Fix fjell-kernel Cargo.toml version pin](./done/003-fix-fjell-kernel-version-pin.md) | v0.1.0 |
| 004 | [Capability gating for TaskSpawn / TaskStart / Lease syscalls](./done/004-capability-gating-task-lease-syscalls.md) | v0.1.0 |
| 005 | [sys_mmio_map — exclude kernel RAM](./done/005-sys-mmio-map-ram-exclusion.md) | v0.1.0 |
| 006 | [LeaseBinding in Capability](./done/006-lease-binding-in-capability.md) | v0.1.0 |
| 007 | [Per-task DMA allocator](./done/007-per-task-dma-allocator.md) | v0.1.0 |
| 008 | [CRC32 for BootControlBlock and StoreSuperblock](./done/008-bootcontrolblock-crc32.md) | v0.1.0 |
| 009 | [W^X enforcement for kernel page table](./done/009-wx-kernel-page-permissions.md) | v0.1.0 |
| 010 | [Generation field in task handle ABI](./done/010-task-handle-generation-abi.md) | v0.1.0 |
| 011 | [Service plane separation](./done/011-service-separation.md) | v0.1.0 |
| 012 | [Real digest verification](./done/012-real-digest-verification.md) | v0.1.0 |
| 013 | [Create ADRs 0006–0010 for M6/M7](./done/013-adr-creation-m6-m7.md) | v0.1.0 |
| 014 | [Complete capability gating](./done/014-complete-capability-gating.md) | v0.1.0 |
| 015 | [Lease validation in IPC / cap paths](./done/015-lease-validation-in-ipc-cap-paths.md) | v0.1.0 |
| 016 | [MmioRegion capability](./done/016-mmio-region-capability.md) | v0.1.0 |
| 017 | [DmaRegion capability](./done/017-dma-region-capability.md) | v0.1.0 |
| 018 | [W^X three-region linker alignment](./done/018-wx-three-region-linker-align.md) | v0.1.0 |
| 019 | [sys_ipc_try_recv + cooperative service loop](./done/019-ipc-try-recv-cooperative-services.md) | v0.1.0 |
| 020 | [Audit drain implementation](./done/020-audit-drain-implementation.md) | v0.1.0 |
| 021 | [cap-broker real policy evaluation](./done/021-cap-broker-policy-evaluation.md) | v0.1.0 |
| 022 | [sys_task_start entry_pc / stack_top validation](./done/022-task-start-entry-validation.md) | v0.1.0 |
| 023 | [BCB mirror selection test](./done/023-bcb-mirror-selection-tests.md) | v0.1.0 |
| 024 | [Release freeze and scope declaration](./done/024-release-freeze-and-scope-declaration.md) | v0.1.0 |
| 025 | [CI / QEMU automation foundation](./done/025-ci-qemu-automation-foundation.md) | v0.1.0 |
| 048 | [Handle-based require_cap for task/lease syscalls](./done/048-handle-based-require-cap-task-lease-syscalls.md) | v0.1.0 |
| 049 | [Capability management rights enforcement](./done/049-capability-management-rights-enforcement.md) | v0.1.0 |
| 050 | [Specific-error-code negative tests](./done/050-specific-error-code-negative-tests.md) | v0.1.0 |
| 051 | [Device VMA range and MMIO mapping correctness](./done/051-device-vma-range-mmio-mapping-correctness.md) | v0.1.0 |
| 052 | [DMA region-table-full rollback and cap-based revoke](./done/052-dma-table-full-rollback-cap-based-revoke.md) | v0.1.0 |
| 053 | [Audit drain no-loss ordering](./done/053-audit-drain-no-loss-ordering.md) | v0.1.0 |
| 054 | [sys_audit_drain unified require_cap](./done/054-audit-drain-unified-require-cap.md) | v0.1.0 |
| 055 | [Kernel-attested sender identity in IPC](./done/055-kernel-attested-sender-identity-ipc.md) | v0.1.0 |
| 056 | [cap-broker capability installation primitive](./done/056-cap-broker-capability-installation-primitive.md) | v0.1.0 |
| 057 | [bootctl service extraction](./done/057-bootctl-service-extraction.md) | v0.1.0 |
| 058 | [service-manager READY tracking](./done/058-service-manager-ready-tracking.md) | v0.1.0 |
| 059 | [v0.2.12 release-gate criteria](./done/059-v0_2_12-release-gate-criteria.md) | v0.1.0 |

### v0.1.x stabilisation (026–030, 044–047)

| ID | Title | Shipped |
|----|-------|---------|
| 026 | [Negative test harness](./done/026-negative-test-harness.md) | v0.1.2 |
| 027 | [Threat model and security boundary documentation](./done/027-threat-model-and-security-boundaries.md) | v0.1.2 |
| 028 | [Syscall, ABI, and protocol inventory](./done/028-syscall-abi-protocol-inventory.md) | v0.1.2 |
| 029 | [Capability / lease enforcement audit](./done/029-capability-lease-enforcement-audit.md) | v0.1.3 |
| 030 | [MMIO / DMA boundary audit](./done/030-mmio-dma-boundary-audit.md) | v0.1.3 |
| 044 | [Audit / snapshot / semantic evidence audit](./done/044-audit-snapshot-semantic-evidence-audit.md) | v0.1.3 |
| 045 | [ADR and documentation synchronization](./done/045-adr-and-documentation-synchronization.md) | v0.1.4 |
| 046 | [v0.1.x release checklist](./done/046-v01x-release-checklist.md) | v0.1.4 |
| 047 | [v0.2 preparation backlog](./done/047-v02-preparation-backlog.md) | v0.1.5 |

### v0.2.0 — Security Boundary Closure (031–043)

| ID | Title | Shipped |
|----|-------|---------|
| 031 | [Unified capability enforcement (require_cap)](./done/031-unified-capability-enforcement.md) | v0.2.0 |
| 032 | [Capability slot drop and CSpace GC](./done/032-capability-slot-drop-cspace-gc.md) | v0.2.0 |
| 033 | [Lease epoch revocation integration](./done/033-lease-epoch-revocation-integration.md) | v0.2.0 |
| 034 | [Blocked IPC revocation semantics](./done/034-blocked-ipc-revocation-semantics.md) | v0.2.0 |
| 035 | [MmioRegion capability and MMIO ABI change](./done/035-mmio-region-capability-abi-change.md) | v0.2.0 |
| 036 | [DmaRegion capability, zeroize, and quarantine](./done/036-dma-region-capability-zeroize-quarantine.md) | v0.2.0 |
| 037 | [Non-blocking IPC, cooperative loop, timer fail-safe](./done/037-non-blocking-ipc-cooperative-loop-timer-fail-safe.md) | v0.2.0 |
| 038 | [Service plane separation foundation](./done/038-service-plane-separation-foundation.md) | v0.2.0 |
| 039 | [Safe user copy and real audit drain](./done/039-safe-user-copy-and-real-audit-drain.md) | v0.2.0 |
| 040 | [cap-broker bootstrap handoff and default-deny](./done/040-cap-broker-bootstrap-default-deny.md) | v0.2.0 |
| 041 | [Persistent evidence hardening](./done/041-persistent-evidence-hardening.md) | v0.2.0 |
| 042 | [v0.2 negative test expansion](./done/042-v02-negative-test-expansion.md) | v0.2.0 |
| 043 | [v0.2 security boundary release gate](./done/043-v02-security-boundary-release-gate.md) | v0.2.0 |

### v0.3.0 — Hardware Trust Abstraction

| RFC | Title | Shipped |
|-----|-------|---------|
| v0.3-001 | [HardwareTrustProvider Interface and Provider Registry](./done/RFC-v0.3-001-hardwaretrustprovider-interface-and-provider-registry.md) | v0.3.0 |
| v0.3-002 | [Keyring, Key Purpose, Signature Provider, and Key Epoch Model](./done/RFC-v0.3-002-keyring-key-purpose-signature-provider-and-key-epoch-model.md) | v0.3.0 |
| v0.3-003 | [Anti-Rollback Metadata and upgraded Confirmation Hardening](./done/RFC-v0.3-003-anti-rollback-metadata-and-upgraded-local-confirmation-hardening.md) | v0.3.0 |
| v0.3-004 | [Local Attestation Profile v2 and Measurement Binding](./done/RFC-v0.3-004-local-attestation-profile-v2-and-measurement-binding.md) | v0.3.0 |

### v0.4.0 — Secure Networking Stack

| RFC | Title | Shipped |
|-----|-------|---------|
| v0.4-001 | [virtio-net User-Space Driver and Network Device Capabilities](./done/RFC-v0.4-001-virtio-net-user-space-driver-and-network-device-capabilities.md) | v0.4.0 |
| v0.4-002 | [Minimal netd Packet and Session Service](./done/RFC-v0.4-002-minimal-netd-packet-and-session-service.md) | v0.4.0 |
| v0.4-003 | [secure-transportd Authenticated Control-Plane Channel](./done/RFC-v0.4-003-secure-transportd-authenticated-control-plane-channel.md) | v0.4.0 |
| v0.4-004 | [Remote Update Metadata Fetch and Staged upgraded Integration](./done/RFC-v0.4-004-remote-update-metadata-fetch-and-staged-upgraded-integration.md) | v0.4.0 |
| v0.4-005 | [Remote Diagnostics and Attestation Transport Foundation](./done/RFC-v0.4-005-remote-diagnostics-and-attestation-transport-foundation.md) | v0.4.0 |

### v0.5.0 — Multi-Platform Foundation and Semantic API Stabilization

| RFC | Title | Shipped |
|-----|-------|---------|
| v0.5-001 | [PlatformProfile and BoardProfile Format](./done/RFC-v0.5-001-platformprofile-and-boardprofile-format.md) | v0.5.0 |
| v0.5-002 | [Device Discovery Strategy — DTB / ACPI and devmgr Boundary](./done/RFC-v0.5-002-device-discovery-strategy-dtb-acpi-and-devmgr-boundary.md) | v0.5.0 |
| v0.5-003 | [Architecture Boundary Cleanup and Second-Platform Preparation](./done/RFC-v0.5-003-architecture-boundary-cleanup-and-second-platform-preparation.md) | v0.5.0 |
| v0.5-004 | [Semantic API Stabilization and Compatibility Policy](./done/RFC-v0.5-004-semantic-api-stabilization-and-compatibility-policy.md) | v0.5.0 |
| v0.5-005 | [Text Proxy Hardening and Critical-Intent Rendering](./done/RFC-v0.5-005-text-proxy-hardening-and-critical-intent-rendering.md) | v0.5.0 |

### v0.6.0 — Verification Hardening

| RFC | Title | Shipped |
|-----|-------|---------|
| v0.6-001 | [Capability, IPC, and Lease Property-Test Harness](./done/RFC-v0.6-001-capability-ipc-and-lease-property-test-harness.md) | v0.6.0 |
| v0.6-002 | [Store Recovery and Boot-Control State-Machine Model Tests](./done/RFC-v0.6-002-store-recovery-and-boot-control-state-machine-model-tests.md) | v0.6.0 |
| v0.6-003 | [Semantic Schema Compatibility and Format Fuzzing](./done/RFC-v0.6-003-semantic-schema-compatibility-and-format-fuzzing.md) | v0.6.0 |
| v0.6-004 | [Unsafe Boundary Inventory and Audit Automation](./done/RFC-v0.6-004-unsafe-boundary-inventory-and-audit-automation.md) | v0.6.0 |

### v0.7.0 — Distributed Snapshot Sync Foundation

| RFC | Title | Shipped |
|-----|-------|---------|
| v0.7-001 | [Node Identity and Snapshot Exchange Trust Model](./done/RFC-v0.7-001-node-identity-and-snapshot-exchange-trust-model.md) | v0.7.0 |
| v0.7-002 | [Signed Snapshot Export and Import Verification](./done/RFC-v0.7-002-signed-snapshot-export-and-import-verification.md) | v0.7.0 |
| v0.7-003 | [Measurement Audit Policy and Release Summary Sync](./done/RFC-v0.7-003-measurement-audit-policy-and-release-summary-sync.md) | v0.7.0 |
| v0.7-004 | [Conflict Domain Metadata and Offline-First Sync Queue](./done/RFC-v0.7-004-conflict-domain-metadata-and-offline-first-sync-queue.md) | v0.7.0 |

### v0.7.x — Incremental hardening patches

| RFC | Title | Shipped |
|-----|-------|---------|
| v0.7.1-001 | [Release Metadata and Reproducibility](./done/RFC-v0.7.1-001-release-metadata-and-reproducibility.md) | v0.7.1 |
| v0.7.1-002 | [CI Coverage and Verification Gate Activation](./done/RFC-v0.7.1-002-ci-coverage-and-verification-gates.md) | v0.7.4 |
| v0.7.1-003 | [Service Plane Integration for v0.4–v0.7 Services](./done/RFC-v0.7.1-003-service-plane-integration.md) | v0.7.1 |
| v0.7.2-001 | [Distributed Sync Service IPC Wiring](./done/RFC-v0.7.2-001-distributed-sync-service-wiring.md) | v0.7.3 |
| v0.7.2-002 | [Snapshot Envelope Size Safety and Conflict-Domain Merge Rules](./done/RFC-v0.7.2-002-snapshot-envelope-safety-and-merge-rules.md) | v0.7.1 |
| v0.7.2-003 | [NodeIdentity Constructor Safety and Trust Mode Fail-Closed](./done/RFC-v0.7.2-003-node-identity-safety-and-trust-mode-fail-closed.md) | v0.7.2 |
| v0.7.3-001 | [v0.4 Networking Runtime Completion](./done/RFC-v0.7.3-001-networking-runtime-completion.md) | v0.7.2 |
| v0.7.3-002 | [Crypto Profile Documentation and Production-Mode Gate](./done/RFC-v0.7.3-002-crypto-profile-documentation.md) | v0.7.1 |
| v0.7.4-001 | [DMA Lifetime Safety](./done/RFC-v0.7.4-001-dma-lifetime-safety.md) | v0.7.1 |
| v0.7.4-002 | [MMIO/DMA Mapping Failure and User-Copy Documentation](./done/RFC-v0.7.4-002-mmio-mapping-failure-and-user-copy-docs.md) | v0.7.1 |
| v0.7.4-003 | [Capability Authority Hardening](./done/RFC-v0.7.4-003-capability-authority-hardening.md) | v0.7.2 |
| v0.7.5-001 | [Semantic Catalog Ownership and Documentation Currency](./done/RFC-v0.7.5-001-catalog-ownership-and-documentation-currency.md) | v0.7.4 |

### v0.8.0 — Fleet / Edge Operations Plane

| RFC | Title | Shipped |
|-----|-------|---------|
| v0.8-001 | [Fleet Identity, Enrollment, and Node Registry](./done/RFC-v0.8-001-fleet-identity-enrollment-and-node-registry.md) | v0.8.0 |
| v0.8-002 | [Semantic State Aggregation and Fleet View](./done/RFC-v0.8-002-semantic-state-aggregation-and-fleet-view.md) | v0.8.0 |
| v0.8-003 | [Staged Rollout Plan and Update Governance](./done/RFC-v0.8-003-staged-rollout-plan-and-update-governance.md) | v0.8.0 |
| v0.8-004 | [Remote Diagnostics and Recovery Intent](./done/RFC-v0.8-004-remote-diagnostics-and-recovery-intent.md) | v0.8.0 |
| v0.8-005 | [Policy Governance and Fleet Policy Distribution](./done/RFC-v0.8-005-policy-governance-and-fleet-policy-distribution.md) | v0.8.0 |

---

## Proposed (proposed/) — 5 RFCs

### v0.9 — Service SDK and Ecosystem

| RFC | Title |
|-----|-------|
| v0.9-001 | [Service SDK and Stable Service API Subset](./proposed/v0.9/RFC-v0.9-001-service-sdk-and-stable-service-api-subset.md) |
| v0.9-002 | [Capability Request Manifest and Policy Lint](./proposed/v0.9/RFC-v0.9-002-capability-request-manifest-and-policy-lint.md) |
| v0.9-003 | [Semantic Node Authoring Toolkit](./proposed/v0.9/RFC-v0.9-003-semantic-node-authoring-toolkit.md) |
| v0.9-004 | [Bundle Builder and Signed Service Package](./proposed/v0.9/RFC-v0.9-004-bundle-builder-and-signed-service-package.md) |
| v0.9-005 | [QEMU Developer Workflow and Service Test Harness](./proposed/v0.9/RFC-v0.9-005-qemu-developer-workflow-and-service-test-harness.md) |

---

## Archive (archive/)

No RFCs withdrawn or superseded.
