# ADR Rename / Migration Note (RFC 045)

This file records the mapping from old ADR filenames (v0.1.0–v0.1.3)
to the mandated set (v0.1.4). It exists so that any external citations
can be updated.

## File rename table

| Old filename | New filename | Action |
|---|---|---|
| `0001-target-architecture.md` | `0001-minimal-microkernel.md` | Superseded; content absorbed into new 0001 |
| `0002-microkernel-boundary.md` | `0001-minimal-microkernel.md` | Superseded; content absorbed into new 0001 |
| `0003-capability-security.md` | `0002-capability-based-ipc.md`, `0003-lease-epoch-revocation.md` | Superseded; content split |
| `0004-semantic-stream.md` | `0005-semantic-stream-first.md` | Renumbered; content extended |
| `0005-v010-scope.md` | — | Historical archive; superseded by RFC 024 |
| `0006-device-driver-model.md` | `0006-user-space-driver-model.md` | Renamed; content extended |
| `0007-persistent-store-model.md` | `0007-append-only-state-store.md` | Renamed; content extended |
| `0008-verified-rootfs-trust-model.md` | `0008-verified-immutable-rootfs.md` | Renamed; content extended |
| `0009-ab-boot-control.md` | `0009-ab-boot-control-health-confirmation.md` | Renamed; content extended |
| `0010-inline-init-workaround.md` | `0010-local-evidence-and-recovery.md` | Superseded by RFC 038; slot reused |
| — | `0011-development-grade-crypto-before-hardware-trust.md` | New |
| — | `0012-no-general-network-before-security-closure.md` | New |
| — | `0004-user-space-service-plane.md` | New |

## Old files retained (marked Superseded)

The old files are kept for historical reference. Each has a
`**Status:** Superseded` header at the top. Do not delete them;
they record the decisions made at each milestone.
