# Architecture Overview

Fjell is a capability-based microkernel system: a small RISC-V supervisor
kernel plus user-mode services that communicate only over kernel-mediated
IPC, with every grant of authority represented as a typed capability.

## Layers

```text
┌────────────────────────────────────────────────────────┐
│ Services (user mode, no_std Rust, one binary each)     │
│  init · configd · cap-broker · auditd · svc-manager    │
│  devmgr · virtio-blk · storaged · bootctl · upgraded … │
├────────────────────────────────────────────────────────┤
│ IPC + capability layer (endpoints, CSpaces, leases)    │
├────────────────────────────────────────────────────────┤
│ Kernel (S-mode, RISC-V Sv39, single-hart at v1.0)      │
│  mm (boot/frame alloc, page tables) · trap · task ·    │
│  lease table · spawn · console                         │
└────────────────────────────────────────────────────────┘
```

## The capability model

A service holds capabilities in its CSpace; a capability names an object
(typically an IPC endpoint) and carries a rights mask. Two invariants are
formally proved in Verus and machine-checked as a release gate:

- **Non-amplification** (`capability`): minting a child capability can never
  add rights the parent lacks (`child & !parent == 0`).
- **Lease epoch revocation** (`lease`): a binding issued at epoch *e* is dead
  after revoke advances the epoch — and the epoch never wraps
  (retire-before-wrap at `u32::MAX`).

See `verification/verus/` and the
[proof gate policy](../../verification/verus/README.md) for the proof layer.

## Boot and updates

Boot control is A/B: two boot-control blocks carry generation counters and
validity, and the selection rule (highest valid generation, deterministic
tiebreak) is the third Verus-proved module. Updates arrive as signed bundles
checked against the keyring with anti-rollback metadata; `upgraded`
orchestrates them and `bootctl` commits the mirror decision.

## Evidence

Every authority grant, update, boot decision, and recovery action emits a
typed semantic record into the audit chain (`auditd`). Release artefacts
include a Trust Report assembled from these records.

## Verification tiers

1. Host library tests and conformance tests (`cargo test`)
2. Property tests (`fjell-proptest`) and fuzz targets
3. Unsafe-site and MMIO-ordering audits (zero-gap gates)
4. Reproducible-build gate (SHA-256, two-build comparison)
5. QEMU smoke (m1–m8 + feature profiles) and negative tests
6. Verus formal proofs (release-required for capability and lease)

`cargo xtask test-all` runs tiers 1–5 locally; `cargo xtask
release-rehearsal` runs the full release gate matrix including the proofs.
