# Summary

[Introduction](./README.md)

---

# New Users (P1)

- [What is Fjell?](./intro/what-is-fjell.md)
- [Why Fjell?](./intro/why-fjell.md)
- [Non-Goals](./intro/non-goals.md)
- [Quick Start](./tutorials/quick-start.md)
- [Three-Node Fleet Tutorial](./tutorials/three-node-fleet.md)
- [FAQ](./faq.md)

---

# Service Authors (P2)

- [SDK Overview](./sdk/overview.md)
- [Writing a Service](./sdk/writing-a-service.md)
- [Capability Manifests](./sdk/cap-manifest.md)
- [Bundle Publishing](./sdk/bundle-publishing.md)
- [Service Cookbook](./sdk/cookbook.md)
- [Intent Catalog v1](./api/semantic-catalog.md)  *(auto-generated from fjell-semantic-v1)*
- [Syscall Reference](./api/syscalls.md)
- [ABI Stability Policy](./abi/policy.md)
- [Stability Tiers](./abi/stability.md)
- [IPC Register Layout](./abi/ipc-register-layout.md)
- [Fjell OS v0.1 ABI / Protocol Inventory](./abi/v0.1-inventory.md)

---

# Reference

- [Audit Event Format](./reference/audit-event-format.md)
- [Capability Model](./reference/capability-model.md)
- [Configuration Format](./reference/configuration-format.md)
- [Intent Stream Schema](./reference/intent-stream-schema.md)
- [IPC Model](./reference/ipc-model.md)
- [Syscall ABI](./reference/syscall-abi.md)

---

# Design and Architecture

- [Requirements Definition](./requirements/requirements-definition.md)
- [Requirements Analysis](./requirements/requirements-analysis.md)
- [External Design](./external-design/README.md)
  - [Kernel](./external-design/kernel.md)
  - [Capability & Lease](./external-design/capability-lease.md)
  - [IPC](./external-design/ipc.md)
  - [Boot & Upgrade](./external-design/boot-upgrade.md)
  - [User-Space Services](./external-design/services.md)
  - [Audit & Observability](./external-design/audit-observability.md)
  - [ABDD / Semantic Streams](./external-design/abdd-semantic.md)
  - [Security & Trust](./external-design/security-trust.md)
  - [Developer Surface](./external-design/developer-surface.md)
- [Capability System](./architecture/capability-system.md)
- [Fleet Operations](./architecture/fleet.md)
- [Leases](./architecture/leases.md)
- [Measurement and Attestation](./architecture/measurement-and-attestation.md)
- [Architecture Overview](./architecture/overview.md)
- [v0.3 RFC-001 — Hardware Trust Provider](./architecture/v0.3-001-trust-provider.md)
- [v0.3 RFC-002 — Keyring, Key-Purpose, Signature-Provider](./architecture/v0.3-002-keyring.md)
- [v1.0 Direction and Identity](./identity/v1-direction.md)

---

# Internals

- [Architecture Overview](./internals/architecture-overview.md)
- [Design Philosophy](./internals/design-philosophy.md)
- [Local Development](./internals/local-development.md)
- [Memory Model](./internals/memory-model.md)
- [Fjell OS — Negative Tests](./internals/negative-tests.md)
- [QEMU Tests](./internals/qemu-tests.md)
- [Task Model](./internals/task-model.md)
- [Trap and Syscall](./internals/trap-syscall.md)
- [`unsafe` Policy](./internals/unsafe-policy.md)

---

# Architecture Decisions

*Every decision, with the ones it replaced kept beneath it.*

- [Architecture Decisions](./adr/README.md)
  - [ADR-0001 — Minimal Microkernel](./adr/0001-minimal-microkernel.md)
  - [ADR-0002 — Capability-Based IPC](./adr/0002-capability-based-ipc.md)
  - [ADR-0003 — Lease Epoch Revocation](./adr/0003-lease-epoch-revocation.md)
  - [ADR-0004 — User-Space Service Plane](./adr/0004-user-space-service-plane.md)
  - [ADR-0005 — Semantic Stream First](./adr/0005-semantic-stream-first.md)
  - [ADR-0006 — User-Space Driver Model](./adr/0006-user-space-driver-model.md)
  - [ADR-0007 — Append-Only State Store](./adr/0007-append-only-state-store.md)
  - [ADR-0008 — Verified Immutable Rootfs](./adr/0008-verified-immutable-rootfs.md)
  - [ADR-0009 — A/B Boot Control and Health Confirmation](./adr/0009-ab-boot-control-health-confirmation.md)
  - [ADR-0010 — Local Evidence and Recovery](./adr/0010-local-evidence-and-recovery.md)
  - [ADR-0011 — Development-Grade Crypto Before Hardware Trust](./adr/0011-development-grade-crypto-before-hardware-trust.md)
  - [ADR-0012 — No General Network Before Security Closure](./adr/0012-no-general-network-before-security-closure.md)
  - [ADR Rename / Migration Note (RFC 045)](./adr/ADR-RENAME.md)
  - [ADR-v0.4-001 — User-space virtio-net Driver with Capability-gated Net Access](./adr/ADR-v0.4-001-user-space-virtio-net-driver.md)
  - [ADR-v0.4-002 — netd: Capability-brokered Session Model for Network Access](./adr/ADR-v0.4-002-netd-session-model.md)
  - [ADR-v0.4-003 — secure-transportd: Single-Suite TLS 1.3 with In-Process Crypto](./adr/ADR-v0.4-003-secure-transportd-single-suite-tls.md)
  - [ADR-v0.4-004 — Operator-Initiated Remote Update Metadata Fetch](./adr/ADR-v0.4-004-operator-initiated-update-fetch.md)
  - [ADR-v0.4-005 — diagnosticsd: Typed Allow-List Redaction and Bundle Authority](./adr/ADR-v0.4-005-diagnosticsd-redaction-and-authority.md)
  - [ADR-v0.5-001 — PlatformProfile / BoardProfile Boundary](./adr/ADR-v0.5-001-platform-board-boundary.md)
  - [ADR-v0.5-002 — No Runtime DTB Parsing in User-Space Services](./adr/ADR-v0.5-002-no-runtime-dtb-parse.md)
  - [ADR-v0.5-003 — Architecture Boundary via Monomorphised Trait](./adr/ADR-v0.5-003-arch-trait-monomorphised.md)
  - [ADR-v0.5-004 — Semantic Catalog v1 Is Frozen](./adr/ADR-v0.5-004-semantic-catalog-v1-frozen.md)
  - [ADR-v0.5-005 — proxy-text Is Output-Only; No Remote Input Path](./adr/ADR-v0.5-005-proxy-text-no-input.md)
  - [ADR-v0.6-001 — Capability/IPC/Lease Property-Test Harness](./adr/ADR-v0.6-001-property-test-harness.md)
  - [ADR-v0.6-002 — Store Recovery and Boot-Control State-Machine Model Tests](./adr/ADR-v0.6-002-store-bootctl-model-tests.md)
  - [ADR-v0.6-003 — Format Fuzzing and Frozen Schema Registry](./adr/ADR-v0.6-003-format-fuzzing.md)
  - [ADR-v0.6-004 — Unsafe Boundary Inventory and Audit Automation](./adr/ADR-v0.6-004-unsafe-audit-automation.md)
  - [ADR-v0.7-001 — Node Identity and Snapshot Exchange Trust Model](./adr/ADR-v0.7-001-node-identity-trust-model.md)
  - [ADR-v0.7-002 — Signed Snapshot Export and Import Verification](./adr/ADR-v0.7-002-signed-snapshot-envelope.md)
  - [ADR-v0.7-003 — Measurement and Release Summary Sync](./adr/ADR-v0.7-003-summary-format.md)
  - [ADR-v0.7-004 — Conflict Domain Metadata and Snapshot v2](./adr/ADR-v0.7-004-conflict-domain-snapshot-v2.md)
  - [Superseded ADRs](./adr/superseded/README.md)
    - [ADR-0001 — Target Architecture: RISC-V 64 + QEMU](./adr/superseded/0001-target-architecture.md)
    - [ADR-0002 — Microkernel Boundary](./adr/superseded/0002-microkernel-boundary.md)
    - [ADR-0003 — Capability-Based Security](./adr/superseded/0003-capability-security.md)
    - [ADR-0004 — Semantic Stream (ABDD)](./adr/superseded/0004-semantic-stream.md)
    - [ADR-0005 — v0.1.0 Scope](./adr/superseded/0005-v010-scope.md)
    - [ADR 0006: User-space driver model and MMIO/DMA capability boundary](./adr/superseded/0006-device-driver-model.md)
    - [ADR 0007: Persistent append-only store and recovery model](./adr/superseded/0007-persistent-store-model.md)
    - [ADR 0008: Verified immutable rootfs and signed artifact model](./adr/superseded/0008-verified-rootfs-trust-model.md)
    - [ADR 0009: A/B boot-control and health-based confirmation model](./adr/superseded/0009-ab-boot-control.md)
    - [ADR 0010: Inline init smoke workaround and service separation deprecation plan](./adr/superseded/0010-inline-init-workaround.md)

---

# Assurance

- [Unsafe Charter](./assurance/unsafe-charter.md)
- [The Unsafe Gate](./assurance/unsafe-gate.md)
- [Property Tests](./assurance/property-tests.md)
- [Verus Setup](./assurance/verus-setup.md)
- [Documentation Structure Audit — 2026-09-16](./assurance/documentation-structure-audit.md)
- [The Instrument Audit's Undisposed Findings — a decision brief](./assurance/instrument-audit-backlog.md)
- [Instrument Audit — Close-Out and Disposition](./assurance/instrument-audit-closeout.md)
- [Instrument Audit Register](./assurance/instrument-audit.md)
- [Fjell OS — MMIO Ordering Audit v0.12](./assurance/mmio-audit-v0.12.md)
- [Verus Proof Gate Policy](./assurance/proofs/proof-gate-policy.md)
  - [Verus Developer Handoff](./assurance/proofs/guides/01-verus-developer-handoff.md)
  - [Verus Proof Authoring Guide](./assurance/proofs/guides/02-proof-authoring-guide.md)
  - [Proof-to-Rust Conformance Workflow](./assurance/proofs/guides/03-proof-to-rust-conformance-workflow.md)
  - [Verus Reviewer Guide](./assurance/proofs/guides/04-reviewer-guide.md)
  - [Migration Guide for Existing Rust Modules](./assurance/proofs/guides/05-migration-guide-for-existing-rust-modules.md)
  - [When Not to Use Verus](./assurance/proofs/guides/06-when-not-to-use-verus.md)
  - [Proof Drift Review Checklist](./assurance/proofs/checklists/proof-drift-review-checklist.md)
  - [Release Proof Gate Checklist](./assurance/proofs/checklists/release-proof-gate-checklist.md)
  - [Verus PR Checklist](./assurance/proofs/checklists/verus-pr-checklist.md)
  - [Proof Review Record: <Target Name>](./assurance/proofs/templates/proof-review-record-template.md)
  - [RFC Supplement: <RFC Title>](./assurance/proofs/templates/rfc-supplement-template.md)
  - [Verus Target Proposal: <Target Name>](./assurance/proofs/templates/verus-target-proposal-template.md)
  - [Proof Review Record: v0.17 Pilot Targets (capability, lease, boot-control)](./assurance/proofs/review-records/v0.17-pilot-targets.md)
  - [Architect Review — v0.18.3 Verus Layer — Decisions Record](./assurance/proofs/review-records/v0.18-architect-review-decisions.md)
  - [Proof-Review Re-Run — Verus prover upgrade (v0.21.3)](./assurance/proofs/review-records/v0.21.3-prover-upgrade.md)
  - [Verus Adoption Risk Register](./assurance/proofs/appendices/A-risk-register.md)
  - [Verus Adoption Anti-Patterns](./assurance/proofs/appendices/B-anti-patterns.md)
  - [Glossary Additions](./assurance/proofs/appendices/C-glossary-additions.md)

---

# Audits

- [Fjell OS v0.1 — Capability / Lease Enforcement Audit](./audit/capability-lease-enforcement-audit-v0.1.md)
- [Fjell OS v0.1 — Audit / Snapshot / Semantic Evidence Export Audit](./audit/evidence-export-audit-v0.1.md)
- [Fjell OS v0.1 — MMIO / DMA Boundary Audit](./audit/mmio-dma-boundary-audit-v0.1.md)

---

# Security and Compliance

- [Adversarial Review — v0.16 Validation Closure](./security/adversarial-review-v0.16.md)
- [Fjell OS v0.1 Threat Model](./security/threat-model-v0.1.md)
- [Fjell OS — Threat Model v1.0](./security/threat-model-v1.md)
- [Fjell OS v0.1.0 — Known Non-Goals](./security/v0.1.0-known-non-goals.md)
- [Fjell OS — Standards Mapping (CRA Annex I / IEC 62443-4-1 / IEC 62443-4-2)](./compliance/standards-mapping.md)

---

# Operations and Deployment

- [Trust Report](./dev/trust-report.md)
- [Developer Modes](./dev/modes.md)
- [Fjell OS — Operator Recovery Guide](./operations/recovery-guide.md)
  - [Recovery Drill Attestation — DR Walkthrough v0.16](./operations/recovery-drills/v0.16-dr-walkthrough.md)
- [Fjell OS — StarFive VisionFive 2 Deployment Guide](./deployment/starfive-visionfive2.md)
- [Performance Baseline](./perf/baseline.md)

---

# Releasing

- [v0 Development Release Cycle](./releasing/v0-release-cycle.md)
- [Fjell OS — v1.0 Release Checklist](./releasing/release-checklist.md)
- [Release Handoff — the standing instruction for cutting a release](./releasing/release-handoff.md)
- [Reproducible Builds](./releasing/reproducibility.md)
- [v1.0 Readiness Matrix](./releasing/v1-readiness.md)
- [v1.0 Limitations — Gate 9 Reference](./releasing/v1-limitations.md)
- [Fjell OS — v1.0 Non-Goals](./releasing/v1-non-goals.md)
- [v1.0 Non-Goals — Adversarial Review](./releasing/v1-non-goals-review.md)
- [Fjell OS v1.0.0 — Release Notes](./releasing/v1.0-release-notes.md)
- [Fjell OS v0.16.0 Release Notes](./releasing/v0.16-release-notes.md)

---

# Roadmap

- [Roadmap and Milestones](./roadmap/roadmap.md)
- [Fjell OS v0.1.x — Stabilisation Roadmap](./roadmap/v0.1.x-stabilization.md)
- [Fjell OS v0.2 — Preparation Backlog](./roadmap/v0.2-preparation-backlog.md)
- [v0.23+ Direction Options](./roadmap/v0.23-direction-options.md)
- [RFC Process](./contributing/rfc-process.md)

---

# Development History

*Engineering session records and the scopes of past milestones, kept for the record.*

- [Session Handoff v0.9–v0.15](./history/handoff-v0.9-v0.15.md)
- [Session Handoff v0.17–v0.18](./history/handoff-v0.17-v0.18.md)
- [Session Handoff v0.19–v0.20](./history/handoff-v0.19-v0.20.md)
- [v1.0 Handoff Bundle](./history/handoff-0.21.2/README.md)
  - [Project Summary](./history/handoff-0.21.2/project-summary.md)
  - [External Design](./history/handoff-0.21.2/external-design.md)
  - [Implementation Notes](./history/handoff-0.21.2/implementation-notes.md)
  - [Testing and Gates](./history/handoff-0.21.2/testing-and-gates.md)
  - [Ops, Release & Security](./history/handoff-0.21.2/ops-security.md)
  - [Decision Log](./history/handoff-0.21.2/decision-log.md)
    - [Evidence](./history/handoff-0.21.2/evidence/README.md)
- [SDK Trial Lessons v0.14](./history/lessons-from-v0.14.md)
- [v0.7 Release Notes](./history/v0.7-release-notes.md)
- [Fjell OS v0.1.0 — Limitations](./history/v0.1.0-limitations.md)
- [Fjell OS v0.1.0 — Scope](./history/v0.1.0-scope.md)
- [Fjell OS v0.1.1 — Developer Summary](./history/v0.1.1-dev-summary.md)
- [Fjell OS v0.1.x Release Checklist](./history/v0.1.x-release-checklist.md)
- [Fjell OS v0.2.0 — Security Boundary Closure Release Gate](./history/v0.2.0-release-gate.md)
- [Fjell OS v0.2 Development Summary](./history/v0.2.x-dev-summary.md)
