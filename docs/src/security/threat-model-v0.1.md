# Fjell OS v0.1 Threat Model

**Version:** v0.1.2 (supersedes the v0.1.1 skeleton).  
**Produced by:** RFC 027 (also known as RFC-v0.1.x-004).  
**Updates to this document** are delivered by: RFC 027 (full body),
RFC 043 (v0.2 gate), and subsequent milestone threat-model updates.

---

## 1. Scope

This threat model covers Fjell OS running on QEMU `virt` (RISC-V,
single hart, no network), evaluated at the v0.1.0 / v0.1.x code base.

The source tree is public; all threat analysis assumes an attacker can
read every line of code.

This is **not** a production threat model. It is an audit baseline to
guide v0.2 boundary closure.

---

## 2. Assets

Assets are the values the system must protect or preserve.

| Asset | Description |
|---|---|
| Kernel memory | Page tables, stacks, data, ELF sections |
| Task address spaces | Isolated user-space memory per task |
| Capability tables | CSpace slots — authority over every kernel object |
| Lease table | Epoch bindings that revoke authority |
| Endpoint table | IPC rendezvous endpoints |
| Call-frame table | In-flight IPC call state |
| DMA regions | Kernel-tracked DMA pages |
| MMIO mappings | Physical device register windows |
| Persistent state store | Append-only durable record log |
| Boot-control block | Active / inactive slot selection, health state |
| Signed release metadata | Bundle digest + signature |
| Signed policy metadata | Cap-broker policy + signature |
| Immutable rootfs metadata | Digest of the verified read-only rootfs |
| Snapshot records | System state projection at a point in time |
| Measurement chain | Ordered sequence of recorded measurements |
| Local attestation records | Signed attestation reports |

---

## 3. Trusted Computing Base (v0.1.0)

The TCB is **maximal** at v0.1.0. Every binary in the prebuilt
`crates/fjell-kernel/prebuilt/` directory is trusted at boot, because
`fjell-init` includes all service images via `build.rs`.

| Component | Status |
|---|---|
| `fjell-kernel` ELF | Fully trusted |
| All service ELFs (20 services) | Fully trusted (embedded in kernel) |
| Development signing key (`dev-attest-m8-01`) | Development-grade only |

**v0.2 note:** Service-plane separation (RFC 038) begins shrinking the
TCB; each separated service's ELF can be individually verified.
Production hardware-rooted trust (TPM / DICE) is a v0.3.0 target.

---

## 4. Attacker Model

### In scope

| Attacker | Description |
|---|---|
| Hostile or buggy user task | A spawned task that invokes syscalls with invalid handles, wrong types, or missing rights |
| Hostile or buggy service | A service that sends malformed IPC or attempts to access ungranted capabilities |

### Out of scope (explicitly deferred)

| Attacker | Deferred to |
|---|---|
| Physical attacker (bus snooping, cold boot) | v0.3 / hardware trust |
| Hardware DMA attacker (rogue device) | v0.3 (IOMMU) |
| Malicious firmware / bootloader | v0.3 |
| Compromised boot ROM | v0.3 |
| Remote attacker over network | v0.4 (after network lands) |
| Supply-chain compromise beyond development-grade signing | v0.3 |
| Side-channel attacks | No current milestone |
| SMP race attacks | When SMP is introduced (no current milestone) |

---

## 5. Trust Boundaries

Each boundary below is *defined* at v0.1.0 but not all are fully
*enforced*. The enforcement column reflects v0.1.0 / v0.1.2 status.

| Boundary | Mechanism | Enforcement |
|---|---|---|
| Kernel ↔ Task | ecall + capability check | Partial (see §6) |
| Service ↔ Service | IPC + capability | Partial (see §7) |
| Service ↔ Driver | MMIO / DMA capability | Partial (see §8) |
| Task ↔ Persistent Store | storaged IPC | Partial |
| Verified Artefact ↔ Recovery | verifyd / recoveryd IPC | Partial |

---

## 6. Capability Boundary

**Purpose:** Only a task holding a valid capability with appropriate
rights may invoke an authority-bearing syscall.

**Current enforcement (v0.1.2):**

- CSpace lookup is real; handles are generation-stamped.
- Kind checks are present on most syscalls (type-level).
- Rights bits (`CapRights`) exist in the ABI (RFC 031 type) but are
  not yet uniformly checked against the required right per operation.
- Lease binding (`LeaseBinding`) exists in the ABI type but is not
  yet consulted on every use path.
- Several syscalls still use `caller_has_cap(kind)` style checks
  rather than `require_cap(task, handle, kind, rights, scope)`.

**Remaining gap:**

The unified `require_cap()` function defined in RFC 031 is not yet
implemented. Until it is, a task that holds *any* capability of the
right kind may call authority-bearing syscalls without the required
right or lease check. This is the primary v0.2 work item.

**v0.2 closure:** RFC 031 implements `require_cap()` and migrates
every syscall path; RFC 032 adds `sys_cap_drop` for CSpace GC;
RFC 033 connects lease epoch checks.

---

## 7. IPC Boundary

**Purpose:** Messages may only be sent/received by the task that holds
the corresponding Endpoint capability with SEND or RECV rights.

**Current enforcement (v0.1.2):**

- Blocking send and receive use real endpoint objects.
- `ipc_try_recv` (non-blocking) exists per RFC 019.
- IPC call / reply use real CallFrame tracking.
- Rights checks (SEND, RECV, CALL, REPLY) are **not yet enforced**
  at the syscall entry; the kind check for `Endpoint` is present but
  rights bits are not consulted.

**Remaining gap:**

An attacker in possession of any Endpoint capability can call
`ipc_send`, `ipc_recv`, `ipc_call`, or `ipc_reply` regardless of
which rights the capability carries.

**v0.2 closure:** RFC 031 (unified enforcement) and RFC 034
(blocked-IPC wake/cancel on revoke).

---

## 8. MMIO / DMA Boundary

**Purpose:** Physical MMIO regions and DMA pages may only be
mapped/allocated by a task holding the correct capability.

**Current enforcement (v0.1.2):**

- `sys_mmio_map` accepts a caller-supplied physical address and size.
  A RAM-exclusion guard (RFC 005) rejects addresses below a hard-coded
  threshold, but this is a coarse defense, not a capability boundary.
- DMA allocation is per-task (RFC 007) but does not require a
  `DmaRegion` capability; ownership is tracked informally.
- There is no `MmioRegionObject`, `DmaRegionObject`, or corresponding
  `CapKind::MmioRegion` / `CapKind::DmaRegion` in v0.1.x.

**Remaining gap:**

Any task that can invoke `sys_mmio_map` can request any
non-RAM physical address. Any task can call `dma_alloc`. Neither
operation is capability-bound at the object level.

**v0.2 closure:** RFC 035 (MmioRegion ABI), RFC 036 (DmaRegion +
zeroize + quarantine).

---

## 9. Persistent Store Boundary

**Purpose:** Only authorised services may write to the durable
append-only state store.

**Current enforcement (v0.1.2):**

- The store record format includes CRC32 (`BootControlBlock` via
  RFC 008) and is rejection-checked on recovery (M5).
- Partial tail records are ignored on recovery.
- The capability required to invoke `storaged`'s store operations is
  tracked in `fjell-service-api` but is a service-level IPC guard,
  not a kernel capability check.

**Remaining gap:**

The store IPC path does not use `require_cap()`. A service that holds
the right endpoint can write to the store without the kernel enforcing
any capability bound.

**v0.2 closure:** RFC 031 + RFC 038 (service-plane separation hardening
the storaged IPC path).

---

## 10. Verified Artefact Boundary

**Purpose:** Release bundles, policy, and rootfs digests must be
verified before use; unverified or tampered artefacts must be rejected.

**Current enforcement (v0.1.2):**

- Release bundle verification via `verifyd` is real (M7).
- Ed25519 stand-in signature (RFC 012) is verified against the
  development key.
- Unsigned bundles are rejected.
- Tampered bundle digests are rejected.
- The signing key is development-grade (in-tree).

**Remaining gap:**

The signing key is not hardware-rooted. An attacker with access to the
source tree can forge any artefact signature by recompiling.

**v0.3 closure:** Hardware Trust Abstraction introduces hardware-rooted
keys.

---

## 11. Recovery Boundary

**Purpose:** Recovery transitions (rollback, re-verify) must be based
on verifiable state, not attacker-controlled data.

**Current enforcement (v0.1.2):**

- Rollback selection logic is in `fjell-recoveryd` (M8).
- Recovery decisions are driven by the snapshot + measurement records.
- Audit events are recorded for rollback selection.

**Remaining gap:**

The recovery evidence chain depends on the measurement chain being
correct; measurement is self-reported (no TPM PCR extend), so a
compromised kernel can lie.

**v0.3 closure:** Hardware Trust Abstraction.

---

## 12. Known Weaknesses

Enumerated by the v0.1.3 audit RFCs (029, 030, 044). The most
critical at v0.1.2:

1. **No `require_cap()`.** Most syscalls use type-only checks;
   rights, scope, and lease are not enforced. (v0.2: RFC 031.)
2. **MMIO accepts arbitrary physical addresses.** Any task can map
   any device region. (v0.2: RFC 035.)
3. **DMA has no capability binding.** Any task can allocate DMA.
   (v0.2: RFC 036.)
4. **Lease revocation is advisory.** Revoking a lease does not
   invalidate existing capabilities on use. (v0.2: RFC 033.)
5. **`cap-broker` is not default-deny.** Services can obtain
   capabilities without policy review. (v0.2: RFC 040.)
6. **`copy_to_user` is unchecked.** No VMA/page-table validation.
   (v0.2: RFC 039.)
7. **Audit drain is a placeholder.** The kernel ring exists but
   `auditd` does not drain real binary records. (v0.2: RFC 039.)
8. **Services are inline `init` logic.** No isolation between
   services. (v0.2: RFC 038.)
9. **Development-grade signing key is in-tree.** Signatures are
   not cryptographically meaningful against an attacker who reads
   the source. (v0.3: Hardware Trust Abstraction.)

---

## 13. Deferred Threats

| Threat | Deferred to |
|---|---|
| Physical attacker | v0.3.0 |
| Hardware DMA attacker | v0.3.0 (IOMMU) |
| Malicious firmware | v0.3.0 |
| Remote attacker | v0.4.0 |
| Side-channel | No current milestone |
| SMP races | No current milestone |
| Supply-chain beyond dev-key | v0.3.0 |

---

## 14. v0.2 Security Boundary Closure Plan

v0.2.0 closes the eight weaknesses listed in §12 via nine phases
and 13 RFCs (031–043). See `docs/roadmap/v0.1.x-stabilization.md`
§v0.2.0 and the v0.2 RFC set for the implementation plan.

The v0.2 acceptance criteria (RFC 043 §gate) are the functional
verification of this threat model's §12 weaknesses having been closed.

After v0.2, the remaining gaps in this model are the v0.3 items:
hardware-rooted trust, IOMMU, and production secure boot.
