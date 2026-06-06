# Fjell OS — v1.0 Direction and Identity

*Source of truth: [RFC 061](../../rfcs/done/061-v1-direction-and-identity.md).
This page is the operator-readable distillation of that decision.*

---

## What Fjell is

> **Fjell OS is a verifiable, capability-based operating system for
> high-assurance edge and fleet nodes where every authority, update,
> recovery action, and runtime state must be explainable.**

"Explainable" means more than readable. It means every authority
grant, update action, and recovery step leaves a signed, typed, and
machine-readable evidence record that can be independently verified.
See [Trust Report](../release/trust-report.md) for the artefact this
produces.

---

## Who Fjell is for — three archetypes

### A1: Industrial gateway

A long-lived control-network gateway in a regulated industrial setting.
Substation telemetry, factory-floor cell controller, water-treatment
bridge. Long operational lifetime; strict change control; every action
traceable to a signed authority.

### A2: Sensor / edge fleet node

A node in a fleet of 10²–10⁵ devices, often power-constrained and
intermittently connected. Environmental monitoring, asset-tracking,
distributed metering. Offline-first; fleet operator needs per-node
attested state without per-node access.

### A3: Regulated field device

A device subject to certification regimes (IEC 62304, IEC 61508, ISO
27001, IEC 62443 adjacent). Medical-adjacent controllers, safety-critical
edge instruments. The compliance auditor must be able to reconstruct any
state from recorded evidence.

---

## What Fjell is not

The following are explicitly **not** targeted before v1.0. See
[v1.0 Non-Goals](../release/v1-non-goals.md) for the full table with
rationale.

- General-purpose servers or web hosting
- Desktop / laptop user environments
- POSIX-compatible OS
- Container orchestration substrate
- Package manager with dependency resolution
- General-purpose remote shell
- Hard real-time scheduling guarantees
- Vehicular / hard-real-time control

---

## Permanent invariants

Eight properties that cannot be weakened. Anything requiring their
removal is, by construction, not a Fjell feature.

| # | Invariant |
|---|-----------|
| I1 | Authority is by capability handle, not ambient identity |
| I2 | Every grant is traceable to a signed authority |
| I3 | Every grant is bounded by a lease with explicit revocation |
| I4 | Every privileged action emits a typed semantic record |
| I5 | Updates are content-addressed, signed, anti-rollback-bound, and locally confirmed |
| I6 | Kernel code is `forbid(unsafe_code)` except behind classified, audited boundaries |
| I7 | No code path bypasses W^X for kernel memory |
| I8 | Recovery from a corrupted root of trust is bounded and auditable |

---

## Networking — what is allowed

Fjell is **not** networkless. What is rejected before v1.0 is
unconstrained default-on networking for arbitrary services. What is
supported: networking via explicit capability grant with declared flows,
service manifests naming expected destinations, and authenticated
control-plane traffic.

The v1.0 invariant: *no service routes packets without holding a
capability that names a specific network device and a specific lease.*

---

## Roadmap to v1.0

| Version | Focus |
|---------|-------|
| v0.10 | Identity, ABI policy, docs, reproducibility — no new runtime features |
| v0.11 | Trust spine hardening — real Ed25519, signing, rotation, replay |
| v0.12 | First real-world deployment profile — one RISC-V target |
| v0.13 | Fleet reliability — partition, key compromise, disaster recovery |
| v0.14 | Developer ecosystem trial — first external service |
| v0.15 | v1.0 freeze candidate — threat model, non-goals, release checklist |
| v1.0  | Discipline release — no new surface |

---

## The Trust Report

Every Fjell release ships a Trust Report: a machine-generated artefact
with six sections that operationalise the "explainable" claim:

1. Capability inventory per service
2. Lease inventory
3. Measurement and bundle digest chain
4. Semantic catalog binding
5. Unsafe site inventory
6. CI evidence (test counts, gates)

See [`cargo xtask trust-report`](../dev/trust-report.md) for usage.

---

## Research track

Speculative work (formal methods, AI-agent authority, energy scheduling,
post-quantum hybrid mode) lives under `research/` and is not mainline.
Promotion to `crates/` requires:

- An RFC in `proposed/`.
- Demonstration against at least one of A1, A2, A3.
- Passing the unsafe-audit, reproducible-build, and test-all gates.
- No relaxation of I1–I8.
