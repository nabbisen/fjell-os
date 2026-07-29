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

`cap_copy` (equal-rights copy), `cap_mint` (strict subset), `cap_revoke`,
`cap_drop`, `cap_install` / `cap_install_with_rights` (broker path),
`cap_inspect`, `cap_bind_lease`.

### Lease operations (syscalls)

`lease_create` (under a `LeaseAdmin` cap), `lease_revoke` (advances epoch),
`lease_inspect` (reads current epoch).

## 3. Requirements coverage

| Requirement | Design response | As-built evidence |
|---|---|---|
| FR-KRN-003 Unforgeable capabilities | Typed handle = (generation, slot); rights mask; kind | `fjell-cap/src/handle.rs`, `rights.rs` |
| FR-KRN-003 delegation / restricted delegation | `cap_copy` (equal), `cap_mint` (subset only) | mint enforces `is_subset_of` |
| FR-KRN-003 revocation | `cap_revoke`; lease revoke cascades | `lease/mod.rs` |
| FR-KRN-003 audit recording | Cap create/mint/revoke/drop emit audit events | `AuditKind::Cap*` |
| FR-SEC-001 Least privilege | Each service receives only the caps it needs at spawn | cap-broker + service manifests |
| FR-SEC-002 Explicit delegation | No implicit delegation; source/target/subject/limits recorded | `cap_mint` + audit |
| FR-SEC-003 Sandboxing | A service's reachable objects = its CSpace contents | per-task CSpace |
| FR-SEC-004 Secure failure | Missing right → `PermissionDenied`, no grant | default-deny mapping |
| NFR-SEC-001 Default deny | Undelegated capability access is rejected | universal in `require_cap_on_ct` |

## 4. Two formally-verified invariants

Machine-checked in Verus and enforced as release gate 10:

- **Non-amplification** (`capability` target): `cap_mint` can never produce a
  child whose rights exceed the parent — `child & !parent == 0`. 8 proof
  obligations.
- **Bounded lease revocation** (`lease` target): the lease epoch is monotonic
  and revocation is bounded; retire-before-wrap at `u32::MAX`. 5 obligations.

These prove the *predicates*. The syscall paths that invoke them are
unit-tested, property-tested, and QEMU-negative-tested — not proven end-to-end
(see [Security & Trust](./security-trust.md) for the verification-scope
statement).

## 5. Lease revocation semantics (external contract)

`sys_lease_revoke` is atomic with respect to in-flight IPC:

1. The lease epoch is advanced (monotonic).
2. Any sender blocked in an endpoint queue with a lease-bound capability is
   cancelled and woken with `Err(LeaseRevoked)`.
3. Any pending reply edge bound to the lease is cleared via
   `cancel_replies_for_lease`; the blocked caller is woken with
   `Err(LeaseRevoked)`.
4. `sys_ipc_reply` additionally checks the reply edge's lease binding
   (defense-in-depth) and refuses if revoked.

This satisfies the requirement that revoked authority must not leave IPC
hanging.

## 6. As-built scope limits & gaps

- Capability persistence across reboot is out of scope for v1.0 (caps are
  bootstrapped fresh at boot).
- `CapInstall` is granted only to cap-broker and init during bootstrap
  (RFC 056); no general capability-installation authority exists in user space.

There are no known correctness gaps in the capability/lease core at v0.21.2; the
Verus proofs and negative tests cover the security-critical predicates.
