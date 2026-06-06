# Summary

[Introduction](./README.md)

---

# Releases

- [v0.1.0 Scope](./releases/v0.1.0-scope.md)
- [v0.1.0 Limitations](./releases/v0.1.0-limitations.md)
- [v0.1.1 Developer Summary](./releases/v0.1.1-dev-summary.md)
- [v0.1.x Release Checklist](./releases/v0.1.x-release-checklist.md)
- [v0.2.0 Release Gate](./releases/v0.2.0-release-gate.md)
- [v0.2.x Developer Summary](./roadmap/v0.2.x-dev-summary.md)

---

# Roadmap

- [v0.1.x Stabilisation](./roadmap/v0.1.x-stabilization.md)
- [v0.2 Preparation Backlog](./roadmap/v0.2-preparation-backlog.md)

---

# Security

- [v0.1.0 Known Non-Goals](./security/v0.1.0-known-non-goals.md)
- [v0.1 Threat Model](./security/threat-model-v0.1.md)

---

# ABI Reference

- [v0.1 ABI Inventory](./abi/v0.1-inventory.md)

---

# Audits

- [Capability / Lease Enforcement Audit](./audit/capability-lease-enforcement-audit-v0.1.md)
- [MMIO / DMA Boundary Audit](./audit/mmio-dma-boundary-audit-v0.1.md)
- [Evidence Export Audit](./audit/evidence-export-audit-v0.1.md)

---

# Development

- [Negative Tests](./development/negative-tests.md)

---

# Architecture Decisions

- [ADR-0001 Minimal Microkernel](./adr/0001-minimal-microkernel.md)
- [ADR-0002 Capability-Based IPC](./adr/0002-capability-based-ipc.md)
- [ADR-0003 Lease Epoch Revocation](./adr/0003-lease-epoch-revocation.md)
- [ADR-0004 User-Space Service Plane](./adr/0004-user-space-service-plane.md)
- [ADR-0005 Semantic Stream First](./adr/0005-semantic-stream-first.md)
- [ADR-0006 User-Space Driver Model](./adr/0006-user-space-driver-model.md)
- [ADR-0007 Append-Only State Store](./adr/0007-append-only-state-store.md)
- [ADR-0008 Verified Immutable Rootfs](./adr/0008-verified-immutable-rootfs.md)
- [ADR-0009 A/B Boot Control](./adr/0009-ab-boot-control-health-confirmation.md)
- [ADR-0010 Local Evidence and Recovery](./adr/0010-local-evidence-and-recovery.md)
- [ADR-0011 Dev-Grade Crypto Before Hardware Trust](./adr/0011-development-grade-crypto-before-hardware-trust.md)
- [ADR-0012 No Network Before Security Closure](./adr/0012-no-general-network-before-security-closure.md)
- [ADR-v0.4-001 User-Space virtio-net Driver](./adr/ADR-v0.4-001-user-space-virtio-net-driver.md)
- [ADR-v0.4-002 netd Session Model](./adr/ADR-v0.4-002-netd-session-model.md)
- [ADR-v0.4-003 secure-transportd Single-Suite TLS](./adr/ADR-v0.4-003-secure-transportd-single-suite-tls.md)
- [ADR-v0.4-004 Operator-Initiated Update Fetch](./adr/ADR-v0.4-004-operator-initiated-update-fetch.md)
- [ADR-v0.4-005 diagnosticsd Redaction and Authority](./adr/ADR-v0.4-005-diagnosticsd-redaction-and-authority.md)
- [ADR-v0.5-001 Platform/Board Boundary](./adr/ADR-v0.5-001-platform-board-boundary.md)
- [ADR-v0.5-002 No Runtime DTB Parsing](./adr/ADR-v0.5-002-no-runtime-dtb-parse.md)
- [ADR-v0.5-003 Arch Trait Monomorphised](./adr/ADR-v0.5-003-arch-trait-monomorphised.md)
- [ADR-v0.5-004 Semantic Catalog v1 Frozen](./adr/ADR-v0.5-004-semantic-catalog-v1-frozen.md)
- [ADR-v0.5-005 proxy-text Output-Only](./adr/ADR-v0.5-005-proxy-text-no-input.md)
- [ADR-v0.6-001 Property-Test Harness](./adr/ADR-v0.6-001-property-test-harness.md)
- [ADR-v0.6-002 Store/Bootctl Model Tests](./adr/ADR-v0.6-002-store-bootctl-model-tests.md)
- [ADR-v0.6-003 Format Fuzzing](./adr/ADR-v0.6-003-format-fuzzing.md)
- [ADR-v0.6-004 Unsafe Audit Automation](./adr/ADR-v0.6-004-unsafe-audit-automation.md)




---

# Getting Started

- [What is Fjell OS?](./getting-started/what-is-fjell-os.md)
- [Quick Start](./getting-started/quick-start.md)
- [FAQ](./getting-started/faq.md)

---

# Reference

- [Syscall ABI](./reference/syscall-abi.md)
- [Capability Model](./reference/capability-model.md)
- [IPC Model](./reference/ipc-model.md)
- [Audit Event Format](./reference/audit-event-format.md)
- [Intent Stream Schema](./reference/intent-stream-schema.md)
- [Configuration Format](./reference/configuration-format.md)

---

# Internals

- [Design Philosophy](./internals/design-philosophy.md)
- [Architecture Overview](./internals/architecture-overview.md)
- [Memory Model](./internals/memory-model.md)
- [Task Model](./internals/task-model.md)
- [Trap and Syscall](./internals/trap-syscall.md)
- [unsafe Policy](./internals/unsafe-policy.md)
- [Local Development](./internals/local-development.md)
- [QEMU Tests](./internals/qemu-tests.md)

---

## Historical ADRs (superseded at v0.1.4 by RFC 045 rename)

- [ADR-0001 — Target Architecture (superseded)](./adr/0001-target-architecture.md)
- [ADR-0002 — Microkernel Boundary (superseded)](./adr/0002-microkernel-boundary.md)
- [ADR-0003 — Capability Security (superseded)](./adr/0003-capability-security.md)
- [ADR-0004 — Semantic Stream (superseded)](./adr/0004-semantic-stream.md)
- [ADR-0005 — v0.1.0 Scope (superseded)](./adr/0005-v010-scope.md)
- [ADR-0006 — Device Driver Model (superseded)](./adr/0006-device-driver-model.md)
- [ADR-0007 — Persistent Store Model (superseded)](./adr/0007-persistent-store-model.md)
- [ADR-0008 — Verified Rootfs Trust Model (superseded)](./adr/0008-verified-rootfs-trust-model.md)
- [ADR-0009 — A/B Boot Control (superseded)](./adr/0009-ab-boot-control.md)
- [ADR-0010 — Inline Init Workaround (superseded)](./adr/0010-inline-init-workaround.md)
