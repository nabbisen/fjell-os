# RFC 061 — Fjell OS v1.0 Direction and Identity

**Status:** Proposed
**Target version:** Decision in v0.10 cycle; commitments bind v0.10 → v1.0.
**Nature:** Identity / constraint RFC. Not an implementation RFC.
**Supersedes:** None.
**Cross-refs:** v0.7.5-001 (catalog ownership), v0.9-001 (SDK),
    v0.9-002 (CapManifest), v0.9-004 (bundles).

## 1. Why this RFC exists

v0.9 closed the architectural surface: capability authority, lease epochs,
attested measurements, signed bundles, fleet ops, semantic catalog, SDK.
The question is no longer *what is there*; it is *what is Fjell for*, and
which adjacent paths are explicitly rejected before v1.0 locks them out.

Without this RFC, every subsequent feature decision reopens the identity
question by accident. POSIX-shim requests will arrive. Browser-host
requests will arrive. Kubernetes-compat requests will arrive. Each is
individually plausible; collectively they would dissolve Fjell into a
second-tier general-purpose Linux alternative. RFC 061 forecloses that
drift while leaving the design room for what Fjell does uniquely.

This is not a marketing exercise. The downstream effect is concrete: it
binds the v0.10–v0.15 RFC sets, the v1.0 ABI freeze scope, the supported
deployment profile, and the threat model.

## 2. Identity statement

> **Fjell OS is a verifiable, capability-based operating system for
> high-assurance edge and fleet nodes where every authority, update,
> recovery action, and runtime state must be explainable.**

"Explainable" is operationalised in §6 below as a renderable artefact, not
a slogan.

## 3. The first serious user — three archetypes

The identity above is too broad to constrain design alone. v0.10 commits
to three named archetypes; design trade-offs are evaluated against this
set, not against the abstract notion of "edge".

### 3.1 Industrial gateway (A1)
A long-lived control-network gateway in a regulated industrial setting.
Examples: substation telemetry concentrator, factory-floor cell
controller, water-treatment SCADA bridge. Characteristics:

- 5–10 year operational lifetime.
- Signed update pipeline with bounded change windows.
- Strict change control; every action must be traceable to a signed
  authority.
- Networking present but **not** general-purpose; flows are declared and
  measured.

### 3.2 Sensor / edge fleet node (A2)
A node in a fleet of 10²–10⁵ devices, often power-constrained, often
intermittently connected. Examples: environmental monitoring,
asset-tracking, distributed metering. Characteristics:

- Offline-first; sync queue is the default state.
- Fleet operator wants per-node attested state without per-node access.
- Compromise of one node must not compromise the fleet.

### 3.3 Regulated field device (A3)
A device subject to certification regimes (IEC 62304, IEC 61508,
DO-178C-adjacent, ISO 27001, IEC 62443). Examples: medical-adjacent
controllers, safety-critical edge instruments. Characteristics:

- Compliance auditor must be able to reconstruct any state from
  recorded evidence.
- Software supply chain must be verifiable end-to-end.
- "No remote shell" is not a feature; it is a regulatory baseline.

### 3.4 Workloads explicitly **not** targeted before v1.0

- General-purpose servers / web hosting.
- Desktop / laptop user environments.
- Mobile consumer devices.
- Container orchestration substrates.
- Vehicular / hard-real-time control (deferred to a post-v1.0 RT profile
  if pursued).

## 4. Permanent invariants

These are non-negotiable for v1.0. Removing or weakening any of them
requires a new identity RFC, not a minor-version change.

| Invariant | Mechanism |
|-----------|-----------|
| **I1.** Authority is by capability handle, not by ambient identity | RFC 031, RFC 048, fjell-cap |
| **I2.** Every grant has a reason traceable to a signed authority | CapBrokerPolicy + manifest (RFC 040, v0.9-002) |
| **I3.** Every grant is bounded by a lease with explicit revocation | RFC 033, RFC 034 |
| **I4.** Every privileged action emits a typed semantic record | Catalog v1 (RFC v0.5-004) |
| **I5.** Updates are content-addressed, signed, anti-rollback-bound, and locally confirmed before commit | RFC v0.3-003, RFC v0.9-004 |
| **I6.** Kernel code is `forbid(unsafe_code)` except behind classified, audited boundaries | UNSAFE_CHARTER + unsafe-audit gate |
| **I7.** No code path bypasses W^X for kernel memory | RFC 009, RFC 018 |
| **I8.** Recovery from a corrupted root of trust is bounded and auditable | RFC v0.3-003, fleet recovery (v0.13) |

Anything that requires weakening I1–I8 to be implemented is, by
construction, not a Fjell feature.

## 5. Provisional commitments

Revisitable on a v0.x → v0.(x+1) boundary with documented rationale.
These are *current best choices*, not invariants.

- **P1.** Primary architecture is RISC-V (`rv64gc`). ARM64 is a
  second-platform target after the first RISC-V real-board deployment.
- **P2.** First supported deployment profile is `qemu-system-riscv64 -M virt`.
  First real-board target picked in v0.12.
- **P3.** Capability authority is mediated by `fjell-cap-broker`; the
  broker's policy language is the file-config form of RFC 040 until
  v0.13 introduces a fleet-distributable form.
- **P4.** Semantic catalog version is v1. Migration to v2, if it
  happens, follows the schema-compatibility policy in RFC-v0.10-002.

## 6. Explainability as a deliverable — the Trust Report

To prevent "explainability" from drifting into rhetoric, every Fjell
release must ship a **Trust Report**: a renderable, version-controlled
artefact describing the running system's authority and evidence
posture.

Mandatory content:

1. **Capability inventory.** For each service: cap-kinds requested
   (from `CapManifest`), cap-kinds granted (from `CapBrokerPolicy`),
   any gaps.
2. **Lease inventory.** For each service: lease kinds held, epoch
   tracking summary, revocation paths.
3. **Measurement chain.** For the release binary: SHA-256 of every
   service binary, anti-rollback metadata, `bundle_digest`.
4. **Semantic catalog binding.** Catalog version, owner crate per
   tag range, semantic schema digest.
5. **Unsafe inventory.** Total unsafe sites with category breakdown.
6. **CI evidence.** Test counts (host, proptest, QEMU), unsafe-audit
   status, reproducible-build status.

The report is generated by `cargo xtask trust-report` and committed
alongside every tagged release. v1.0 ships it; v0.10 lands its
first machine-generated version.

## 7. Networking — disambiguating "no arbitrary networking"

Fjell is **not** networkless. v0.4 provides netd, virtio-net, secure
transport. What is rejected before v1.0:

- ✗ Unconstrained, default-on networking for arbitrary services.
- ✗ A service obtaining `CapKind::NetDevice` by virtue of running.

What is supported:

- ✓ Networking via explicit capability grant with declared flows.
- ✓ Service manifests that name expected outbound destinations.
- ✓ Measured, attested control-plane traffic
  (secure-transportd, RFC v0.4-003).
- ✓ Fleet sync and remote-diagnostics traffic over the same authenticated
  channel.

The v1.0 invariant: *no service routes packets without holding a
capability that names a specific network device and a specific lease.*

## 8. Research → mainline promotion gate

Speculative work (formal methods, AI-agent authority models, energy
scheduling, etc.) is welcome but must not leak into mainline by
accident. The gate:

1. Research lives in a sibling workspace (`research/`) with its own
   Cargo manifest, not in `crates/`.
2. Promotion to `crates/` requires:
   - An RFC in `proposed/` describing the surface and trade-offs.
   - Demonstration against at least one of A1, A2, A3.
   - Passing the unsafe-audit gate, the reproducible-build gate, and
     the test-all gate.
3. No research code path silently relaxes I1–I8.

## 9. ABI freeze scope for v1.0

This is the surface that v1.0 freezes; anything outside this list is
non-stable.

| Layer | Frozen at v1.0 | Status today |
|-------|----------------|--------------|
| User syscall ABI (`fjell-syscall`) | Yes — SDK_API_REV bound | Drafted, tested |
| Capability kinds & rights (`fjell-cap`) | Yes | Stable since v0.2 |
| Lease epoch semantics | Yes | Stable since v0.2 |
| Semantic catalog v1 layout & tags | Yes — frozen v1 | Frozen by RFC v0.5-004 |
| Audit record binary format | Yes | Stable since v0.2 |
| Bundle wire format & digest | Yes | Stable since v0.9 |
| Service IPC tags in `fjell-service-api::v0_7` | Yes | Drafted v0.7 |
| Boot control block format | Yes | Stable since v0.1 |
| CapManifest TOML grammar | Yes | Drafted v0.9 |
| --- | --- | --- |
| Internal kernel APIs (`crates/fjell-kernel/src/**`) | **No** | Internal |
| Service implementation crates | **No** | Internal |
| Internal cap-broker policy file format | Provisional | Likely v1.1+ |
| Fleet protocol bytes | Provisional | Likely v1.1+ |

## 10. Roadmap shape (v0.10 → v1.0)

| Version | Focus | Posture |
|---------|-------|---------|
| v0.10 | Identity, ABI policy, docs, reproducibility | No new runtime features |
| v0.11 | Trust spine hardening (real crypto, key rotation, replay) | Trust ↑, surface unchanged |
| v0.12 | First real-world deployment profile (target chosen) | Reach ↑ |
| v0.13 | Fleet reliability and recovery depth | Operability ↑ |
| v0.14 | Developer ecosystem trial (first external service) | SDK validated |
| v0.15 | v1.0 freeze candidate | No new surface |
| v1.0  | Release | Discipline, not architecture |

## 11. Acceptance criteria

This RFC moves to `done/` when:

1. The v0.10 RFC set (RFCs v0.10-001 through v0.10-007) is drafted in
   `proposed/v0.10/` and references this RFC.
2. The identity statement in §2, the three archetypes in §3, the
   invariants in §4, the non-goals in §3.4, and the ABI freeze scope
   in §9 appear in `docs/src/identity/v1-direction.md` and pass review.
3. `cargo xtask trust-report --dry-run` produces all six sections of §6
   from real workspace data, even if some fields are stubs.
4. The next contentious feature request (POSIX-shim, browser, package
   manager, remote shell) is closed by reference to this RFC, not by
   ad-hoc debate.

## 12. Open questions

Captured here, deferred to the v0.10 set or to v0.11–v0.15:

- Does v1.0 require ARM64 boot? (Deferred to v0.12.)
- Should the cap-broker policy file format be frozen at v1.0 or v1.1?
  (Provisional in §9; revisit in RFC-v0.10-002.)
- Does the Trust Report cover historical evidence, or only the current
  release? (Initial answer: current release only; historical chain is
  v0.13.)
- Is there an LTS branch model? (Deferred until first external user.)

## 13. What this RFC is *not*

- Not a technical implementation plan.
- Not a marketing position.
- Not a final commitment for v2.0+ — only v1.0.
- Not a license or governance change.
- Not a contributor invitation policy.
