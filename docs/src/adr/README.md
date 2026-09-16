# Architecture Decisions

Every architectural decision this project has taken, with the reasoning that
led to it. An ADR is a record: it is not edited once accepted, and a decision
that changes is replaced by a new ADR rather than by an edit to the old one.

The ones that were replaced are kept — see
[Superseded ADRs](./superseded/README.md) — because the reason a decision was
reversed is usually more useful than the decision itself.

| Decision | Status |
|---|---|
| [ADR-0001 — Minimal Microkernel](./0001-minimal-microkernel.md) | Accepted |
| [ADR-0002 — Capability-Based IPC](./0002-capability-based-ipc.md) | Accepted |
| [ADR-0003 — Lease Epoch Revocation](./0003-lease-epoch-revocation.md) | Accepted |
| [ADR-0004 — User-Space Service Plane](./0004-user-space-service-plane.md) | Accepted |
| [ADR-0005 — Semantic Stream First](./0005-semantic-stream-first.md) | Accepted |
| [ADR-0006 — User-Space Driver Model](./0006-user-space-driver-model.md) | Accepted |
| [ADR-0007 — Append-Only State Store](./0007-append-only-state-store.md) | Accepted |
| [ADR-0008 — Verified Immutable Rootfs](./0008-verified-immutable-rootfs.md) | Accepted |
| [ADR-0009 — A/B Boot Control and Health Confirmation](./0009-ab-boot-control-health-confirmation.md) | Accepted |
| [ADR-0010 — Local Evidence and Recovery](./0010-local-evidence-and-recovery.md) | Accepted |
| [ADR-0011 — Development-Grade Crypto Before Hardware Trust](./0011-development-grade-crypto-before-hardware-trust.md) | Accepted |
| [ADR-0012 — No General Network Before Security Closure](./0012-no-general-network-before-security-closure.md) | Accepted |
| [ADR Rename / Migration Note (RFC 045)](./ADR-RENAME.md) | Superseded |
| [ADR-v0.4-001 — User-space virtio-net Driver with Capability-gated Net Access](./ADR-v0.4-001-user-space-virtio-net-driver.md) | Accepted |
| [ADR-v0.4-002 — netd: Capability-brokered Session Model for Network Access](./ADR-v0.4-002-netd-session-model.md) | Accepted |
| [ADR-v0.4-003 — secure-transportd: Single-Suite TLS 1.3 with In-Process Crypto](./ADR-v0.4-003-secure-transportd-single-suite-tls.md) | Accepted |
| [ADR-v0.4-004 — Operator-Initiated Remote Update Metadata Fetch](./ADR-v0.4-004-operator-initiated-update-fetch.md) | Accepted |
| [ADR-v0.4-005 — diagnosticsd: Typed Allow-List Redaction and Bundle Authority](./ADR-v0.4-005-diagnosticsd-redaction-and-authority.md) | Accepted |
| [ADR-v0.5-001 — PlatformProfile / BoardProfile Boundary](./ADR-v0.5-001-platform-board-boundary.md) | Accepted |
| [ADR-v0.5-002 — No Runtime DTB Parsing in User-Space Services](./ADR-v0.5-002-no-runtime-dtb-parse.md) | Accepted |
| [ADR-v0.5-003 — Architecture Boundary via Monomorphised Trait](./ADR-v0.5-003-arch-trait-monomorphised.md) | Accepted |
| [ADR-v0.5-004 — Semantic Catalog v1 Is Frozen](./ADR-v0.5-004-semantic-catalog-v1-frozen.md) | Accepted |
| [ADR-v0.5-005 — proxy-text Is Output-Only; No Remote Input Path](./ADR-v0.5-005-proxy-text-no-input.md) | Accepted |
| [ADR-v0.6-001 — Capability/IPC/Lease Property-Test Harness](./ADR-v0.6-001-property-test-harness.md) | Accepted |
| [ADR-v0.6-002 — Store Recovery and Boot-Control State-Machine Model Tests](./ADR-v0.6-002-store-bootctl-model-tests.md) | Accepted |
| [ADR-v0.6-003 — Format Fuzzing and Frozen Schema Registry](./ADR-v0.6-003-format-fuzzing.md) | Accepted |
| [ADR-v0.6-004 — Unsafe Boundary Inventory and Audit Automation](./ADR-v0.6-004-unsafe-audit-automation.md) | Accepted |
| [ADR-v0.7-001 — Node Identity and Snapshot Exchange Trust Model](./ADR-v0.7-001-node-identity-trust-model.md) | Accepted |
| [ADR-v0.7-002 — Signed Snapshot Export and Import Verification](./ADR-v0.7-002-signed-snapshot-envelope.md) | Accepted |
| [ADR-v0.7-003 — Measurement and Release Summary Sync](./ADR-v0.7-003-summary-format.md) | Accepted |
| [ADR-v0.7-004 — Conflict Domain Metadata and Snapshot v2](./ADR-v0.7-004-conflict-domain-snapshot-v2.md) | Accepted |
