# Fjell OS — v1.0 Non-Goals

*Governed by RFC-v0.15-005. Authoritative for v1.0.0. Each item is
individually justified below. Changes require an identity-level RFC.*

---

## N1 — POSIX compatibility surface

**Fjell does not provide:** `read(2)`, `write(2)`, `open(2)`, `fork(2)`, file descriptors, signals, process groups, ttys, or any POSIX-compatible runtime.

**Why rejected:** POSIX descriptors are ambient authority. Shipping a POSIX shim defeats invariant I1 (authority by capability handle, not ambient identity).

**Why tempting:** Enormous existing software ecosystem; would broaden applicability immediately.

**Operator alternative:** Run POSIX software on Linux or BSD on adjacent hardware. Fjell is for new services, not ported ones.

**Reconsideration:** A research-track RFC could explore a "POSIX-shaped service" that translates descriptor operations into capability-bearing IPC while preserving I1. Not scheduled.

---

## N2 — Default-on networking for arbitrary services

**Fjell does not provide:** Any service can read/write the network without explicit capability grant.

**What Fjell does provide:** Networking via explicit capability grant with declared flows (RFC v0.4).

**Why rejected:** Unconstrained network access violates I1.

---

## N3 — Desktop GUI or web browser hosting

**Fjell does not provide:** Any windowing system, GPU driver, or browser runtime.

**Why rejected:** Fjell targets headless edge/fleet nodes (A1/A2/A3). A GUI stack would dwarf the kernel surface.

---

## N4 — Package manager with dependency resolution

**Fjell does not provide:** Recursive dependency resolution, version constraint solving, or a dependency graph.

**What Fjell does provide:** A file-based artifact registry with flat versioning (RFC-v0.14-004).

**Why rejected:** Dependency resolution introduces ambient transitive authority. Each Fjell bundle is self-contained by design.

---

## N5 — General-purpose remote shell

**Fjell does not provide:** SSH, rsh, or any shell-level remote access.

**Why rejected:** A shell bypasses the capability system. Operator interaction is through signed bundles and fleet management commands.

---

## N6 — Container orchestration substrate

**Fjell does not provide:** Kubernetes-compatible APIs, pod scheduling, or container runtime.

**Why rejected:** Container orchestration assumes ambient POSIX. Incompatible with I1.

---

## N7 — Hard real-time scheduling guarantees

**Fjell does not provide:** Deterministic response-time bounds, RTOS-class scheduling, or certified preemption latency.

**Why rejected:** Requires a fundamentally different scheduler design. Fjell's cooperative+quantum scheduler is unsuitable for hard-RT without significant rearchitecture.

---

## N8 — Multi-fleet federation

**Fjell does not provide:** Cross-organisation trust anchor exchange, inter-fleet authority delegation, or federated update distribution.

**Why rejected:** The v1.0 threat model is bounded to a single fleet. Federation introduces cross-org trust policy with no current operational consumer.

**Reconsideration:** Requires an identity-level RFC analogous to RFC 061 but for federation, plus a new threat model.

---

## N9 — Automatic leader election / coordinator failover

**Fjell does not provide:** Automatic promotion of a surviving member to coordinator when the current coordinator is unavailable.

**What Fjell does provide:** Operator-driven promotion via `TrustAnchorRoot` signature (RFC-v0.13-005 §4).

**Why rejected:** Automatic leader election requires distributed consensus (Raft/Paxos). Deferred to v1.x research. The v1.0 invariant: a fleet without a reachable coordinator is visibly so and refuses to fabricate authority.

---

## N10 — WASM workload hosting

**Fjell does not provide:** A WebAssembly runtime or WASM-based sandbox.

**Why rejected:** WASM hosting is a speculative feature without a current A1/A2/A3 use case. Adds substantial surface.

---

## N11 — Heterogeneous accelerator support (GPU/NPU/DSP)

**Fjell does not provide:** GPU compute, ML accelerator, or DSP driver frameworks.

**Why rejected:** Accelerators have complex, vendor-specific MMUs and driver models incompatible with the current MMIO ownership model.

---

## N12 — Multi-language SDK bindings

**Fjell does not provide:** C, C++, Python, or other language SDKs.

**Why rejected:** Rust only for v1.0. Multi-language FFI complicates the capability safety guarantees.

---

## N13 — Public service marketplace

**Fjell does not provide:** A hosted registry, discovery service, or monetisation platform for third-party bundles.

**Why rejected:** The v0.14 registry is file-based and operator-local. A public marketplace changes trust boundaries.

---

## N14 — AI agent autonomous authority

**Fjell does not provide:** Any mechanism for an AI agent to acquire, delegate, or exercise capabilities autonomously outside the normal capability-grant path.

**Why rejected:** Autonomous authority acquisition directly violates I1 and I2. AI agents may exist as Fjell services within the capability model; autonomous authority does not follow.

---

## N15 — Live service code patching

**Fjell does not provide:** Hot-patching, dynamic reloading, or any mechanism to modify a running service's code without a full bundle lifecycle.

**Why rejected:** Live patching undermines the measurement chain (T8 defences). All code changes must go through the bundle FSM.

---

## N16 — Cryptographic algorithm hot-swap

**Fjell does not provide:** Runtime algorithm negotiation or in-flight cipher suite migration.

**Why rejected:** The signing algorithm (Ed25519) is fixed at v1.0. A post-quantum hybrid mode requires a new RFC and a coordinated fleet migration.

---

## N17 — Distributed consensus across nodes

**Fjell does not provide:** Raft, Paxos, or any replicated state machine.

**Why rejected:** Distributed consensus requires a different failure model than Fjell's current single-coordinator topology. Deferred to post-v1.0 research.

---

## N18 — ARM64 as a supported platform

**Fjell does not provide:** An ARM64 build target, board profile, or deployment guide at v1.0.

**Note:** The architecture trait (`fjell-arch`) already accommodates ARM64; a second platform is an explicit v1.x milestone. The `platform/` tree has a placeholder slot.

---

## N19 — LTS branch and long-term support policy

**Fjell does not provide:** An LTS branch, guaranteed backport policy, or EOL schedule.

**Why rejected:** v1.0 is the first stable release. LTS policy requires operational experience with the release cadence. Deferred post-v1.0.

---

## N20 — Self-healing without operator approval

**Fjell does not provide:** Automatic recovery that changes authority (key rotation, coordinator promotion, policy changes) without operator confirmation.

**Why rejected:** Every authority change must be explainable and attributed. Automatic authority changes, however bounded, undermine that claim.

**What Fjell does provide:** Automatic retry of bounded, non-authority-changing operations (e.g. retrying a failed measurement upload).

---

*Adversarial review attested at `docs/release/v1-non-goals-review.md`.*
