# RFC-v0.12-001 — Deployment Profile Hardening Overview

**Status:** Implemented (v0.12.0)
**Target version:** v0.12.0
**Parent:** RFC 061 §10 (roadmap), §3 archetypes A1/A2/A3.
**Cross-refs:** v0.12-002 through v0.12-005.

## 1. Purpose

Every Fjell test today runs on `qemu-system-riscv64 -M virt`. RFC 061
commits Fjell to high-assurance edge / fleet workloads, but the OS has
never executed on physical silicon. The credibility gap matters: v0.5
architectural portability and v0.7 device discovery are unproven outside
emulation.

v0.12 closes that gap by *one supported profile beyond QEMU `virt`*.
The choice is conservative: a single, fully documented hardware target
(or a stricter QEMU profile if real hardware proves impractical), not
a board portfolio. Multi-board support is an explicit v1.x+ concern.

What "supported" means here: anyone following the v0.10-006 deployment
docs reaches `TEST:V0.12-PROFILE:PASS` on the chosen target without
out-of-band help.

## 2. Composition

| RFC | Title | Deliverable |
|-----|-------|-------------|
| v0.12-001 | This overview | Coordination |
| v0.12-002 | Real-Board Target Selection (or Hardened QEMU Fallback) | Decision + `BoardProfile` |
| v0.12-003 | DTB and Boot Handoff Validation | Boot-time DTB checker |
| v0.12-004 | Interrupt and MMIO Ordering Audit | Audit report + fence audit pass |
| v0.12-005 | Field Operations Notes and Deployment Guide | `docs/deployment/<target>.md` |

The work is sequential: target choice (002) gates everything that
references its devices, ordering, and firmware behaviour.

## 3. Posture

v0.12 deliberately *adds reach without adding surface*. No new SDK
items, no new IPC tags, no new catalog entries except those needed to
attest the new boot profile (one or two `PLATFORM.*` intents).

Real hardware exposes assumptions QEMU hides. The expected outcome is
not "everything works the same" — it is "every place where it
*doesn't* work the same becomes an explicit invariant or a documented
limitation." v0.12 is as much an audit milestone as a deployment one.

## 4. Profile fork: hardware vs hardened-QEMU

The principal decision in v0.12-002 is between:

- **Path A (hardware):** Pick a RISC-V board, validate Fjell against
  it, ship boot media instructions. Highest credibility.
- **Path B (hardened QEMU):** Pick a QEMU machine type beyond `virt`
  (e.g. `sifive_u`) with a stricter, more realistic device set; treat
  it as the v0.12 "deployment profile." Lower credibility but
  reproducible without procuring hardware.

v0.12-001 recommends Path A by default. Path B is acceptable only with
explicit rationale in v0.12-002 and a Path A milestone scheduled for
v0.13 or earlier.

## 5. Release criteria

v0.12.0 may be tagged when:

1. The v0.12 sub-RFCs (002–005) are merged to `done/`.
2. A `BoardProfile` for the chosen target is committed.
3. A documented procedure produces bootable media from a clean checkout.
4. Booting that media on the chosen target reaches `TEST:V0.12-PROFILE:PASS`
   end-to-end.
5. The interrupt/MMIO ordering audit (v0.12-004) report is committed
   and any fixes it identifies have landed.
6. `cargo xtask test-all` continues to pass on QEMU `virt`. The new
   profile is opt-in (it requires hardware; CI cannot run it by default).
7. The Trust Report gains a "Deployment profile" subsection naming the
   target, firmware, and validated invariants.

## 6. Risk register

| Risk | Mitigation |
|------|------------|
| Selected board ages out of supply | v0.12-002 names a primary + secondary candidate |
| Firmware variability across board revisions | DTB validator (v0.12-003) refuses unrecognised configurations |
| Subtle ordering bugs hide on QEMU | v0.12-004 explicit audit; not "we'll test on hardware" |
| Operational toil deploying to hardware | v0.12-005 documents the minimum-friction workflow |
| Scope creep toward multi-board portfolio | This RFC rejects it; one target only |

## 7. Out of scope

- Multi-board support. The architecture trait (`fjell-arch`) is ready
  for ARM64; landing a second platform is v1.x or post-v1.0.
- A driver framework for arbitrary devices. v0.12 supports only what
  the chosen target requires (storage, console, optional networking,
  optional RTC).
- Bootloader development. Fjell uses what the platform provides
  (OpenSBI / U-Boot on RISC-V); v0.12 does not ship a custom loader.
- Userland power-management beyond what v0.6 `powerd` already supports.
- Production manufacturing / provisioning workflows (post-v1.0).
