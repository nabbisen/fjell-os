# Fjell OS

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Build](https://github.com/nabbisen/fjell-os/actions/workflows/ci.yml/badge.svg)](https://github.com/nabbisen/fjell-os/actions/workflows/ci.yml)
[![Version](https://img.shields.io/badge/version-0.15.1-blue.svg)](CHANGELOG.md)

> **Every authority is explainable. Every update is verifiable. Every failure is recoverable.**

---

## Overview

Fjell OS is a capability-based microkernel for high-assurance edge and fleet nodes.
Written in Rust 2024 edition — `forbid(unsafe_code)` except at 268 audited,
classified boundaries.

Current version: **v0.15.1** — v1.0 freeze candidate (patch).

---

## Why / When

Fjell is for operators who need to answer three questions about every node in their fleet:

- **What is running?** — Every deployed binary is content-addressed and signed.
- **Who authorised it?** — Every capability grant has a traceable, leased provenance.
- **How do I recover?** — Every documented failure mode has a tested playbook.

Primary archetypes: industrial gateway (A1), sensor/edge fleet node (A2), regulated field device (A3).

Not for: general-purpose servers, desktop environments, POSIX-compatible workloads.
See [v1.0 Non-Goals](docs/release/v1-non-goals.md).

---

## Quick Start

```bash
# Build
cargo xtask build

# QEMU smoke test
cargo xtask qemu-test m8
# → TEST:M8:PASS

# Three-node fleet demo
cargo xtask fleet-demo deploy
# → TEST:V0.10-FLEET-DEMO:PASS

# Trust Report
cargo xtask trust-report --dry-run
```

---

## Design Notes

| Property | Implementation |
|----------|----------------|
| Authority model | Capability handles — no ambient authority, no `root` |
| Update integrity | Signed bundles, content-addressed, anti-rollback (RFC v0.3) |
| Trust spine | Ed25519 (RFC 8032), key rotation, replay cache (v0.11) |
| Attestation | `AttestationRecordV2` — hash chain, signed, nonce-protected |
| Fleet partition | Reconcile manifests, coordinator-required promotion (v0.13) |
| Unsafe discipline | 268 classified sites, zero missing SAFETY comments |
| MMIO discipline | 23 sites, all classified and annotated |

---

## More Detail

- [Full documentation](docs/src/SUMMARY.md)
- [v1.0 Identity and Direction](docs/src/identity/v1-direction.md)
- [v1.0 Readiness Matrix](docs/release/v1-readiness.md) — 51 DONE, 0 OPEN
- [v1.0 Non-Goals](docs/release/v1-non-goals.md) — 20 explicitly scoped items
- [Threat Model](docs/security/threat-model-v1.md) — 20 in-scope threats
- [Trust Report](docs/release/v1.0.0/trust-report.txt)
- [RFC Process](rfcs/README.md) — 139 RFCs in `done/`
- [Deployment: StarFive VisionFive 2](docs/deployment/starfive-visionfive2.md)
- [Fleet Demo Tutorial](examples/three-node-fleet/README.md)
- [Performance Baseline](docs/perf/baseline.md)
- [MMIO Audit Report](docs/verification/mmio-audit-v0.12.md)

---

## License

Apache-2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).  
Author: nabbisen
