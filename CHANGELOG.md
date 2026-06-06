# Fjell OS — Changelog

All notable changes to this project are documented in this file.
Versions follow `MAJOR.MINOR.PATCH` semantics from v1.0.0 onward.

---

## [0.15.1] — 2026-05-28

**Freeze candidate patch.** README, CHANGELOG, and readiness-matrix polish.
v1.0.0 tag pending owner approval.

All v1.0 propositions satisfied (RFC 061 §4):
identity locked, ABI frozen, trust spine production-grade,
first real-world deployment profile, fleet recovery depth,
SDK trial complete, threat model finalised.

### Milestones completed in this release line

| Milestone | Summary |
|-----------|---------|
| v0.10 | ABI snapshot gate, reproducible builds, criterion benchmarks, three-node fleet demo, mdbook docs, v1.0 readiness matrix |
| v0.11 | Ed25519 signature backend (RFC 8032), bundle signing pipeline, keyring rotation + revocation records, replay cache + nonce table |
| v0.12 | StarFive VisionFive 2 board profile, DTB validation at boot, MMIO ordering audit (23 sites, all classified), deployment guide |
| v0.13 | Fleet partition FSM, reconcile manifests, coordinator promotion, bulk re-attestation, disaster recovery patterns, summary consistency checker |
| v0.14 | `fjell-config-sync` reference service, typed catalog struct generation, bundle publishing registry, developer modes (`--trace`, `--measure`, `--gdb`) |
| v0.15 | Threat model v1 (20 in-scope threats), release checklist, security advisory process, operator recovery guide, non-goals lock (20 items) |

### Final state at v1.0.0

- **564 host tests**, 0 failures
- **139 RFCs** in `done/`
- **268 unsafe sites**, 0 missing SAFETY comments
- **23 MMIO sites**, 0 missing annotations
- **401-item ABI snapshot**, verify gate passes
- **v1.0 readiness matrix**: 51 DONE, 3 DEFERRED, 0 OPEN
- **Trust Report**: 6 sections populated
- **Deployment target**: StarFive VisionFive 2 (primary), QEMU `virt` (CI)

### Breaking changes

None relative to v0.9.x — the v0.10 ABI snapshot captures the stable
surface; no STABLE items were removed or renamed during v0.10–v1.0.

---

## Previous releases

See `docs/src/releases/` for v0.1.x–v0.9.x release notes.

---
