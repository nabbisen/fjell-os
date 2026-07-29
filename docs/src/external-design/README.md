# External Design

*The external design (basic design) tier of Fjell OS. It sits between the
[Requirements Definition](../requirements/requirements-definition.md) and the
per-milestone internal (detailed) design. It describes each subsystem from the
outside inward — its responsibilities, its external contracts, and how it
satisfies the FR/NFR requirements — mapped to the v0.21.2 as-built codebase.*

## Purpose and scope

This document answers, per subsystem: what is its responsibility, what does it
expose at its boundary, which requirements does it satisfy, and what did the
implementation actually build. It is anchored to **both** the requirements
(citing FR-*/NFR-* identifiers) and the **as-built code** (citing crates and
modules at v0.21.2). Where the two diverge, the divergence is called out as a
known gap.

External design fixes *boundaries and contracts*. Internal mechanics (data
structures, algorithms) belong to the per-milestone internal design documents
(`fjell-os-v0_1_0-mN-内部設計書`) and to the architecture reference under
`docs/src/architecture/`.

## Subsystems

| # | Subsystem | Covers | Primary requirements |
|---|---|---|---|
| 1 | [Kernel](./kernel.md) | Microkernel core: syscalls, address spaces, scheduling, interrupts | FR-KRN-001…007, NFR-SEC, NFR-VER, NFR-PERF |
| 2 | [Capability & Lease](./capability-lease.md) | Authority model: capability kinds, rights, delegation, lease revocation | FR-KRN-003, FR-SEC-001…004, NFR-SEC-001 |
| 3 | [IPC](./ipc.md) | Synchronous rendezvous IPC, register ABI, typed messages | FR-KRN-004, NFR-PERF-001 |
| 4 | [Boot & Upgrade](./boot-upgrade.md) | Verifiable boot, minimal init set, A/B atomic upgrade, rollback | FR-BOOT-001…003, NFR-REL-003 |
| 5 | [User-Space Services](./services.md) | The service plane: drivers, storage, config, device manager | FR-SVC-001…006, NFR-REL-001/002 |
| 6 | [Audit & Observability](./audit-observability.md) | Continuous audit API, append-only store, plain-text export | FR-AUD-001…005, NFR-SEC-003/004 |
| 7 | [ABDD / Semantic Streams](./abdd-semantic.md) | Intent Stream, Presentation Proxy boundary, semantic schema | FR-SEM-001…005, NFR-ACC-001…004 |
| 8 | [Security & Trust](./security-trust.md) | Signed bundles, trust anchor, crypto boundary, secure failure | FR-SEC-004, NFR-SEC-001…004, FR-SEM-004 |
| 9 | [Developer Surface](./developer-surface.md) | SDK, semantic schema publication, verifiable interface definitions | FR-DEV-001…003, NFR-MNT |

## Reading order

New readers should start with [Kernel](./kernel.md) and
[Capability & Lease](./capability-lease.md) — they define the trust model every
other subsystem depends on. Service authors will find
[Services](./services.md), [ABDD / Semantic Streams](./abdd-semantic.md), and
[Developer Surface](./developer-surface.md) most relevant. Auditors and
operators should read [Audit & Observability](./audit-observability.md) and
[Security & Trust](./security-trust.md).

## Conventions

- **Requirement mapping tables** cite the requirement ID and the crate/module
  that satisfies it.
- **As-built** notes describe what v0.21.2 actually ships, which may be a subset
  of the full requirement (v1.0 is a scoped QEMU profile). Scope limits are
  flagged, not hidden.
- Crate paths reflect the v0.21.0 subdirectory layout (`crates/formats/`,
  `crates/services/`, etc.).
