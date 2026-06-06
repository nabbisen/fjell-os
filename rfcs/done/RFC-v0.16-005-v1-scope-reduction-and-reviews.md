# RFC-v0.16-005: v1.0 Scope Reduction and Adversarial Reviews

**Status:** Implemented (v0.16.0)
**Milestone:** v0.16 — Validation Closure
**Addresses:** architect review RB-02 (Option B), H-02; errata E-004, E-007, E-009

## 1. v1.0 supported-profile narrowing

Per architect Option B, v1.0 is scoped as a **QEMU `virt` supported
profile**, not a validated-hardware release. The supported claim:

> Fjell OS v1.0 is the first supported QEMU RISC-V (`virt`) profile for a
> capability-based, semantically observable, signed-bundle, fleet-oriented
> OS prototype with enforced local security boundaries and validated QEMU
> reference workflows.

The StarFive VisionFive 2 profile is retained as **provisional and
unvalidated on silicon** (errata E-004, ACCEPTED).

## 2. Claims v1.0 must NOT make

Recorded in `docs/release/v1-non-goals.md` and the release notes:

- validated real-hardware deployment
- production industrial-gateway readiness
- fully rehearsed fleet disaster recovery (only DR1/DR2/DR5 + partition drilled)
- hardware-rooted trust (TrustAnchorRoot provisioning undefined — H-02)
- multi-hart safety (single-hart only)
- POSIX compatibility
- kernel-mediated IPC for the SDK reference service (handler-level only)

## 3. Adversarial reviews (errata E-007, E-009)

Recorded review pass over the threat model and non-goals; findings folded
back. See `docs/security/adversarial-review-v0.16.md`.

## 4. Trust-anchor provisioning (H-02)

Provisioning a `TrustAnchorRoot` onto a fresh node is acknowledged as
undefined and deferred to a dedicated RFC:
**RFC-v0.17-001: Trust Anchor Provisioning and Manufacturing Flow.**
Listed as a v1.0 limitation.
