# External Design — Capability & Lease

*Subsystem 2 of 9. Anchored to FR-KRN-003, FR-SEC-001…004, NFR-SEC-001 and the
`fjell-cap` crate + `fjell-kernel/src/lease/` at v0.21.2.*

## 1. Responsibility

This subsystem is the authority model. It replaces the traditional `root`
premise (FR-KRN-003, "will not do" 4.4) with unforgeable capabilities: a task
may act on memory, devices, IPC endpoints, files, state, and audit APIs only
within the scope of capabilities explicitly held or delegated. Leases add a
time/lifecycle dimension so authority can be bounded and revoked atomically.

## 2. External surface

### Capability kinds (as-built, `fjell-cap/src/rights.rs`)

`Endpoint`, `Reply`, `TaskControl`, `TaskCreate`, `TaskInspect`, `LeaseAdmin`,
`MmioRegion`, `DmaRegion`, `AuditDrain`, `BootEvidence`, `Reboot`, `CapInstall`,
`PersistentStore`, `BootControl`, `UpgradeTransaction`, `Verification`, plus the
rootfs/immutable-namespace kinds. Each names an object and carries a rights
mask.

### Rights (as-built)

A 25+-bit rights lattice: `READ`, `WRITE`, `EXECUTE`, `SEND`, `RECV`, `CALL`,
`REPLY`, `COPY`, `MINT`, `REVOKE`, `INSPECT`, `DROP`, `TASK_CREATE`,
`TASK_START`, `TASK_STATUS`, `TASK_KILL`, `LEASE_CREATE`, `LEASE_REVOKE`,
`LEASE_INSPECT`, `MMIO_MAP`, `DMA_ALLOC`, `DMA_USE`, `DMA_REVOKE`,
`AUDIT_DRAIN`, `BOOT_READ`, …

### Capability operations (syscalls)

`cap_copy` (equal-rights copy), `cap_mint` (strict subset), `cap_delete`,
`cap_revoke`, `cap_drop`, `cap_inspect`, `cap_bind_lease`.

`cap_bind_lease` binds a capability the caller owns to a lease. The caller must hold
an `Active` `LeaseAdmin` capability carrying `LEASE_CREATE` (otherwise
`PermissionDenied`) and the lease must be `Active` (otherwise `LeaseRevoked`) —
RFC-0.38-002. It replaces any earlier binding of that capability.

`cap_install` installs a capability into the CSpace of **the task the caller is
serving** -- the caller of its pending IPC reply edge. It has no target argument,
so a stale or forged target cannot be named (RFC-0.45-002, which made RFC 056's
primitive true; `cap_install_with_rights`, which discarded its rights parameter,
is gone). The caller must hold a `CapInstall` capability carrying `CAP_INSTALL`
(granted **by name**, in no aggregate: `ALL_NON_META` excludes it); the kernel
installs `Endpoint` capabilities only, with exactly the rights asked for, inside
the installer's envelope, **always bound to a lease the installer owns**, and
audits every install (`cap.installed`, `0x0601`) and every refusal
(`cap.install_rejected`, `0x0602`). The one holder is `cap-broker`, whose grant
is therefore lease-bound (`BROKER-004`): revoking the lease kills the
capability. Bootstrap installation at spawn still goes through the in-kernel
`CSpace::install_raw` path, called directly from `main.rs`.


## 6. As-built scope limits & gaps

- Capability persistence across reboot is out of scope for v1.0 (caps are
  bootstrapped fresh at boot).
- Runtime capability installation is narrow by design (RFC-0.45-002): `Endpoint`
  only, into the task being served, lease-bound, and one holder (`cap-broker`).
  Other kinds (`TaskControl`, `MmioRegion`, ...) are refused until a scoped
  design and a consumer exist. The broker's `SxtSession` class names no object
  yet, so `secure-transportd`'s `CAP_SESSION` delivery still cannot happen.
  Lease slots are recycled (RFC-0.46-001): a revoked slot is reclaimed on allocation
  pressure and its generation bumped, a task owns at most 16 `Active` leases, and the
  broker grants a requester at most 4 at a time.

There are no known correctness gaps in the capability/lease core at v0.21.2; the
Verus proofs and negative tests cover the security-critical predicates.
