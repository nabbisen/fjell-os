# RFC-v0.12-004 — Interrupt and MMIO Ordering Audit

**Status:** Proposed
**Target version:** v0.12.0
**Parent:** v0.12-001.
**Cross-refs:** RFC 016 (MmioRegion), v0.5-001..003, v0.12-002.

## 1. Problem

QEMU `virt` provides MMIO that is effectively sequentially consistent
from a guest's point of view; real RISC-V hardware does not. RVWMO
(the RISC-V Weak Memory Ordering model) allows reorderings of loads
and stores, including between MMIO regions, that QEMU rarely exposes.
A driver that runs correctly under emulation may fail on hardware for
reasons no test covers.

v0.12 conducts an *explicit, recorded* audit of every MMIO access path
in the kernel and userspace drivers, classifies it, and adds the
fences the audit identifies as missing.

The objective is not "as fast as possible." It is "every MMIO
ordering decision is *named* somewhere in the code and corresponds to
the documented intent."

## 2. Scope

In scope:

- All MMIO accesses through `MmioRegion` (RFC 016) in kernel.
- All MMIO accesses inside drivers under `crates/fjell-driver-*`.
- All interrupt-controller (PLIC) interactions.
- All DMA descriptor writes that synchronise with device-visible
  state (RFC 017 / 036).

Out of scope:

- Userspace IPC ordering (already governed by the IPC ABI).
- File-system or persistent-store ordering (already RFC 053 territory).
- Inter-hart ordering (single-hart for v1.0 by §6).

## 3. Audit procedure

The audit is mechanical, not heuristic:

1. **Inventory.** A new tool `tools/fjell-mmio-audit/` scans all
   `crates/` source for:
   - `ptr::read_volatile` / `ptr::write_volatile` calls.
   - `core::sync::atomic` operations with non-`SeqCst` ordering.
   - Calls to fence intrinsics (`fence`, `compiler_fence`).
   - Use of `MmioRegion` methods.
   Every site is recorded with file:line and a unique audit id.

2. **Classify.** Each site is annotated with one of:
   - `// MMIO-ORDER: device_setup` — pre-start config; weaker
     ordering allowed.
   - `// MMIO-ORDER: device_kick` — store that releases work; must be
     preceded by a release fence.
   - `// MMIO-ORDER: descriptor_publish` — store that publishes a
     descriptor; must be preceded by a release fence and an `fence rw,w`.
   - `// MMIO-ORDER: status_read` — load that observes device state;
     must be followed by an acquire fence if subsequent loads depend on it.
   - `// MMIO-ORDER: irq_ack` — write that clears an interrupt; must
     synchronise with any subsequent re-enable.
   - `// MMIO-ORDER: poll` — busy-wait read; reordering benign.

3. **Justify.** Each annotation must point to either the RVWMO clause
   or the device datasheet section that motivates the chosen fence
   placement.

4. **Enforce.** A new CI gate `ci-mmio-audit` runs the tool with
   `--check`: every MMIO site must carry an `MMIO-ORDER:` annotation,
   exactly as `ci-unsafe-audit` (RFC v0.6-004 + RFC 060) handles
   `SAFETY:`.

## 4. Likely fixes

The audit will probably surface (a representative, not exhaustive,
list — actual findings published with v0.12 landing):

- Missing `fence rw,rw` after virtio descriptor publication before
  notification kick.
- Reliance on `release` ordering where `release + fence rw,w` is
  needed for MMIO visibility.
- IRQ-ack writes followed by re-enable reads without a `fence w,r`.
- PLIC pending/claim ordering subtleties on real hardware vs. QEMU.

Each finding is documented in `docs/verification/mmio-audit-v0.12.md`
with the audit id, before/after, and the rationale.

## 5. Documentation deliverable

`docs/verification/mmio-audit-v0.12.md` contains:

- Audit tool description.
- Inventory at landing time (total sites, per-classification counts).
- Every finding with file:line, classification, justification, and the
  fix applied.
- A pinned summary in the Trust Report (RFC 061 §6) including:
  - Total sites.
  - Per-classification counts.
  - Audit-tool gate status (pass/fail).

## 6. Single-hart restriction

v1.0 explicitly targets single-hart operation. Multi-hart introduces:

- Inter-hart ordering on shared regions.
- TLB shootdown / IPI subtleties.
- More complex IRQ routing through the PLIC.

This audit assumes single-hart and is sound only for that
configuration. The kernel's existing platform code already pins to
hart 0; the audit explicitly documents this restriction in §1 of the
audit report, and v0.13 (or a later RFC) takes up multi-hart with a
fresh audit.

## 7. Acceptance criteria

1. `tools/fjell-mmio-audit/` exists with a `--check` mode that scans
   the workspace.
2. Every MMIO site in the workspace carries an `MMIO-ORDER:`
   annotation; `--check` exits 0.
3. `ci-mmio-audit` is wired into CI and into the host tier of
   `cargo xtask test-all`.
4. `docs/verification/mmio-audit-v0.12.md` is committed with the full
   inventory and findings.
5. Trust Report includes the audit summary subsection.
6. On the chosen target (v0.12-002), the kernel boots and exercises
   storage + console without ordering-induced failures across at least
   100 reboot cycles.
7. The new annotation rule is documented in `docs/contributing/`.

## 8. Out of scope

- Formal hardware model verification (research track).
- Multi-hart correctness (v0.13+ if pursued).
- Fence-elimination optimisation. Correctness first; performance
  tuning is a separate RFC after baseline numbers stabilise.
- Memory-barrier coverage in cryptographic primitives — already
  handled by the audited crates (v0.11-002).
