# RFC-v0.10-001 — v0.10 Release Maturity Overview

**Status:** Proposed
**Target version:** v0.10.0
**Parent:** RFC 061 (v1.0 Direction and Identity).
**Cross-refs:** v0.10-002 through v0.10-007.

## 1. Purpose

v0.10 is the **maturity milestone**. The goal is not to add runtime
features; it is to make the system that exists at v0.9 *understandable,
measurable, releasable, and credible* — so that v0.11 onwards can build
the trust spine on a stable foundation.

After v0.10, an outside reader should be able to:

1. Find out what Fjell is for, in one document (RFC 061 + identity guide).
2. Know which APIs are frozen and which are not (RFC-v0.10-002).
3. Build a release bit-for-bit reproducibly (RFC-v0.10-003).
4. See the cost of every operation against a baseline (RFC-v0.10-004).
5. Deploy a working three-node fleet in QEMU by following one tutorial
   (RFC-v0.10-005).
6. Find an answer to most operational questions in `docs/` (RFC-v0.10-006).
7. Read a checklist of what v1.0 has not yet achieved (RFC-v0.10-007).

## 2. Composition

| RFC | Title | Deliverable |
|-----|-------|-------------|
| v0.10-001 | This overview | Coordination |
| v0.10-002 | ABI and Semantic Schema Compatibility Policy | `docs/abi/policy.md` + CI gate |
| v0.10-003 | Reproducible Build and Release Gate | Determinism check in `test-all` |
| v0.10-004 | Benchmark Baseline and Regression Tracking | `cargo xtask bench`, baseline file |
| v0.10-005 | Reference QEMU Fleet Deployment | `examples/three-node-fleet/` |
| v0.10-006 | Documentation Maturity and Persona Guides | `docs/src/` complete for three personas |
| v0.10-007 | v1.0 Readiness Matrix | `docs/release/v1-readiness.md` |

These RFCs are independent but should land in roughly the listed order;
each later RFC references earlier ones.

## 3. What v0.10 explicitly does **not** include

- New runtime features (deferred to v0.11+).
- Real cryptographic providers (deferred to v0.11).
- Real-hardware support (deferred to v0.12).
- New fleet protocols (deferred to v0.13).
- External-developer onboarding programs (deferred to v0.14).

This non-goals list is enforced. A change that fits a later milestone
better should not land in v0.10 even if it is small.

## 4. v0.10 release criteria

The v0.10.0 tag may be cut only when:

1. RFC 061 has been merged to `done/`.
2. All six sub-RFCs (v0.10-002 through v0.10-007) are merged to `done/`.
3. `cargo xtask test-all` passes on a clean checkout (host + QEMU).
4. `cargo xtask bench` produces a baseline file checked into the repo.
5. `cargo xtask trust-report` produces a non-empty report.
6. `docs/src/` builds without warnings under `mdbook build`.
7. The three-node QEMU tutorial runs end-to-end on a developer machine
   in under fifteen minutes.

## 5. Risk register

| Risk | Mitigation |
|------|------------|
| "One more small feature" pressure | RFC 061 §3.4 / §10 list — hard reject |
| Doc work outpaces verifier reality | Every doc claim must reference a passing test or marked TODO |
| Benchmark noise masks regression | RFC-v0.10-004 specifies tolerance bands |
| Reproducibility gaps in `prebuilt/` bins | RFC-v0.10-003 makes prebuilts re-derivable |
| Reference fleet brittleness | RFC-v0.10-005 pins exact versions + seeds |

## 6. Estimated duration

Roughly one development cycle (about the size of v0.7 or v0.8).
v0.10 is wide but shallow; each sub-RFC is a small, independent
deliverable.
