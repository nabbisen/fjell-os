# RFC-v0.12-005 — Field Operations Notes and Deployment Guide

**Status:** Implemented (v0.12.0)
**Target version:** v0.12.0
**Parent:** v0.12-001.
**Cross-refs:** v0.12-002 (target), v0.12-003 (DTB validation),
    RFC-v0.10-006 (persona docs).

## 1. Problem

A target choice and a passing boot test are not enough. An operator
who has never seen Fjell must be able to take a published release,
follow one document, and put the OS on a device. Without that document
the v0.12 milestone is a private demo.

This RFC defines the deployment guide as a *committed artefact* with
explicit success criteria.

## 2. Deliverable

A single document at `docs/deployment/<target>.md` covering:

### 2.1 Prerequisites

- Hardware specification (which board revision, which firmware
  version range, which serial cable).
- Host build environment (Rust toolchain version per
  `rust-toolchain.toml`, required system packages).
- Storage media specification (SD card class / size, eMMC partition
  layout if applicable).
- Power and physical connectivity assumptions.

### 2.2 Build

- One command path from clean checkout to a flashable image. Reuses
  `cargo xtask release` from v0.10-003 wherever possible.
- Verification of the image against published digests (signed via
  v0.11-003 by v0.11 landing time).

### 2.3 Flash

- Step-by-step write to storage media.
- Verification that the media is good before insertion.
- Recovery from a failed flash.

### 2.4 First boot

- Expected console output line by line for the first minute.
- Expected DTB validation success message (v0.12-003).
- Expected service-manager READY tags.
- Common first-boot failures and their diagnostics.

### 2.5 Diagnostics

- How to read the audit ring from console.
- How to capture the Trust Report from the running system.
- How to recover from a boot failure (rollback via `bootctl`,
  RFC 057).

### 2.6 Decommissioning

- How to securely wipe persistent state.
- Which evidence to retain for audit, which to destroy.

## 3. Truth bond

Per RFC-v0.10-006 §3: every command and output line in the document
either corresponds to a checked-in test fixture or carries an explicit
TODO referencing the RFC that will resolve it. The document is not a
work of fiction.

A fixture verifier `tools/fjell-doc-fixtures/` extracts code blocks
marked with a fenced-block info string (`bash run-verified`) and
asserts that running them on the chosen target's bring-up environment
produces the documented output. This runs in CI for QEMU; on real
hardware it is operator-attested at landing time.

## 4. Failure modes table

The guide includes a fixed table of common failure modes with
diagnostic and resolution:

| Symptom on console | Diagnosis | Resolution |
|--------------------|-----------|------------|
| Nothing in 10s after firmware banner | Image not loaded | Re-flash, verify digest |
| `FJELL-BOOT-FAIL: DTB` | DTB mismatch | Check firmware version against §2.1 |
| `kernel-mode fault: StorePageFault` | Memory map mismatch | File issue with full DTB dump |
| Boots to `init: ready` then hangs | Service spawn failure | Capture audit ring; check service-manager READY status |
| Reboots in a loop | Recovery path active | Run `bootctl status`; see RFC 057 |

Each row links to a longer treatment in the doc or a relevant RFC.

## 5. CI and verification

- Documents are built by `cargo xtask docs build` (v0.10-006).
- Code-block fixtures run in CI for QEMU paths.
- Real-hardware paths require explicit operator attestation at landing;
  the form is a small checklist committed to `docs/deployment/<target>-attestation-<vN>.md`
  carrying the attesting operator's signature (under the v0.11 keyring).

## 6. Operator interaction with the trust spine

The deployment guide is the first time an operator interacts with the
v0.11 trust spine in anger. The guide covers:

- Where the trust anchors come from (RFC-v0.11-004).
- How to verify the release bundle signature locally before flashing
  (RFC-v0.11-003).
- What to do if signature verification fails.
- How to rotate trust anchors on a deployed device.

These sections explicitly do not paper over the rough edges; they
document them, and link to the RFCs that will smooth them in v0.13.

## 7. Acceptance criteria

1. `docs/deployment/<target>.md` exists, covering all of §2.
2. `cargo xtask docs build` passes including fixture verification.
3. An outside operator can deploy Fjell to the chosen target by
   following the guide alone. ("Outside" means not the document
   author; verified at landing by one independent attempt.)
4. The §4 failure-modes table covers each documented diagnostic
   produced by v0.12-003 and the boot pipeline.
5. The trust-spine §6 sections are present and reference v0.11 RFCs.
6. A signed attestation of the doc's accuracy is committed at landing.

## 8. Out of scope

- Multi-device fleet provisioning (v0.13).
- Manufacturing / factory provisioning (post-v1.0).
- Video walkthroughs.
- Localisation. The guide is English only for v0.12.
