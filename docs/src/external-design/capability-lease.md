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
`cap_revoke`, `cap_drop`, `cap_inspect`, `cap_bind_lease`, `cap_install`.

`cap_install` installs a capability into another task's CSpace. It is dispatched since 0.45 and installs `Endpoint` capabilities only (RFC-0.45-002); `cap-broker` is its one holder (see below).

`cap_bind_lease` binds a capability the caller owns to a lease. **Authority is by
possession, and the call names no authorising handle** (RFC-0.47-001 R4): the caller must
hold an `Active` `LeaseAdmin` capability carrying **both `LEASE_CREATE` and `LEASE_BIND`**,
whose own lease is live (otherwise `PermissionDenied`), and the lease being bound must be
`Active` (otherwise `LeaseRevoked`) — RFC-0.38-002. Since 0.47 this is `require_held`, one
entry of the single authority funnel, where it used to be a hand-rolled scan of the CSpace
repeating four of the funnel's steps by hand.

**A binding is set once, and `cap_bind_lease` refuses a capability that already carries one**
— even to the same lease (RFC-0.38-004, `crates/fjell-cap/src/cspace.rs:181`). *Corrected
2026-11-23 at the 0.47.0 cut, by exit criterion 8: this paragraph said it "replaces any
earlier binding of that capability", which is **the opposite** of what the kernel does and
had been since 0.38 — nine milestones in a published page, stating that an authority could be
moved off its lease when the kernel exists to refuse exactly that. It also named only
`LEASE_CREATE`. The claim's inverse is what E-090 was filed and closed for.*

`cap_install` installs a capability into the CSpace of **the task the caller is
serving** -- the caller of its pending IPC reply edge. It has no target argument,
so a stale or forged target cannot be named (RFC-0.45-002, which made RFC 056's
primitive true; `cap_install_with_rights`, which discarded its rights parameter,
is gone). The caller must hold a `CapInstall` capability carrying `CAP_INSTALL`
(granted **by name**, in no aggregate: `ALL_NON_META` excludes it); the kernel
installs `Endpoint` capabilities only, with exactly the rights asked for, inside
the installer's envelope **and inside the rights of a source capability the installer
names in its own CSpace** (RFC-0.46-002: there is no object argument -- the object, kind
and ceiling are taken from the capability the installer *holds*, so authority is derived
and attenuated, never minted from an integer), **always bound to a lease the installer
owns**, and
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
  Since RFC-0.46-002 an install **derives** from a capability the installer holds: the call
  names a source and `kind` and `object` are read from it, so a forged object is
  unrepresentable rather than rejected — at the price that the broker now *holds* what it
  delegates, bounded by the reviewed `BROKERED_ENDPOINTS` table.
  Lease slots are recycled (RFC-0.46-001): a revoked slot is reclaimed on allocation
  pressure and its generation bumped, a task owns at most 16 `Active` leases, and the
  broker grants a requester at most 4 at a time.
- **Every task's boot capabilities are bound to a boot lease its spawner owns** (RFC-0.48-001).
  `spawn()` creates the lease before any frame is allocated, and one pass binds every capability
  the task holds, so authority granted at boot **can be withdrawn without ending the task** —
  which was not true before 0.48, when the kernel's own grants carried no lease and
  `RFC-v0.7.4-003`'s premise that every meaningful capability has one was unenforced. The lease
  is revoked when the subject exits or faults, and **not** when its owner dies: `init` exiting
  must not strip every service it started. `MAX_LEASES` is **128**, holding one lease per task
  plus a delegator's whole quota. Two fixtures keep slot 0 unbound by design — `neg-test` and
  `sample-service` bind capabilities to leases *they* create, and a capability binds once — each
  named with its reason in the `bootstrap-leases` subcheck.
  **What revocation does not reach:** authority exercised **per call** is withdrawn; authority
  that became a **mapping** is not. A revoked `MmioRegion` stops further mapping **and, since 0.49,
  stops its holder unmapping** (`sys_mmio_unmap` goes through the same funnel), and the pages
  already mapped stay until the holder ends or a holder with a live capability unmaps them.
  `MmioUnmap` is dispatched and verified (RFC-0.49-002), but it is **the holder's act**: lease
  revocation reclaims neither a DMA frame nor an MMIO mapping. `bootstrap-leases` now guards that
  reason rather than the old one — every call to a device-unmap primitive must sit in a classified
  function (the holder's syscalls, a task's death, a failed map's own rollback), so the day lease
  revocation acquires such a call this paragraph has to be restated.
- **A lease is revoked by its owner** (RFC-0.46-003). Every `LeaseAdmin` carries
  `ObjectScope::Any` — necessarily, since the leases they revoke are created at runtime — so
  the kernel checks ownership rather than scope. One named exception, `LEASE_REVOKE_ANY`, is
  held by `neg-test` alone, because an owner blocked on its own leased capability cannot
  revoke it and only a non-owner can wake it; `capability-topology` counts the holders.

**Correctness gaps: as of 0.46.0, none known in the capability/lease core**, and the Verus
proofs and negative tests cover the security-critical predicates — the lease model now
machine-checks slot reuse as well, at 11 obligations against 5.

*Re-dated 2026-11-21 at the 0.46.0 cut, by exit criterion 8. This read "no known correctness
gaps … **at v0.21.2**" — a claim twenty-five milestones old, sitting in an as-built section where
it reads as current. It was also not true for the whole of that period: **E-110** recorded a
reachable denial of the lease table, closed by RFC-0.46-001 two commits before this edit. A
statement of this kind must carry the version it was last taken at, or it is the E-014 class.*
