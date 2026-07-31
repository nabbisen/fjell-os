# Fjell OS Roadmap

Development proceeds as a series of focused milestones.  Each milestone
produces a named release archive.  No milestone stretches into the territory
of the next; scope discipline is a first-class constraint.

---

## v0.1.0 — Initial Release

### M0 · Repository Foundation ✅
- Cargo workspace with all crate skeletons
- `no_std` kernel crate, panic handler
- Documentation skeleton, ADR template
- CI pipeline skeleton
- `LICENSE`, `NOTICE`, `TERMS_OF_USE.md`

### M1 · Bootable Kernel
- Linker script (`link.ld`) for QEMU `virt` RAM at `0x8000_0000`
- `_start` assembly: hart selection, BSS clear, stack pointer
- UART 16550A driver (MMIO `0x1000_0000`)
- `kmain()` prints boot banner
- `cargo xtask qemu` runner

### M2 · Memory and Task Isolation
- M-mode shim → S-mode kernel handoff
- DTB-based physical memory discovery
- `BootAllocator` + bitmap `FrameAllocator`
- Sv39 page tables; shared kernel map + per-task user maps
- `TrapFrame`, `KernelContext`, `Task`, `TaskTable`
- Fixed-priority round-robin scheduler, idle task
- `sys_yield`, `sys_exit`
- User page-fault containment → `TaskState::Faulted`
- QEMU smoke test: `TEST:M2:PASS`

### M3 · IPC and Capability
- Synchronous rendezvous `Endpoint`
- `Capability`, `CapRights`, generation-tagged `CapHandle`
- Derivation tree, `cap_copy / cap_mint / cap_delete / cap_revoke`
- `ipc_send / ipc_recv / ipc_call / ipc_reply`
- One-shot reply edge
- Audit hooks for cap / IPC events
- QEMU smoke test: `TEST:M3:PASS`

### M4 · init / service-manager
- `fjell-init` user-space service
- `fjell-service-manager` with TOML service manifest
- Sample service lifecycle (start / exit / fault)

### M5 · Audit and State Export
- `AuditEvent` ring flush to `fjell-auditd`
- JSON Lines export
- `previous_hash` chain for tamper evidence

### M6 · Declarative Configuration
- TOML config schema + validation
- Dry-run, apply, rollback metadata

### M7 · Semantic Stream and Text Proxy
- `IntentNode` full schema
- `fjell-proxy-text` renderer
- `fjell-sample-service` emits intent

### M8 · v0.1.0 Hardening
- Property tests (`proptest`) for cap / IPC / scheduler
- Full unsafe audit with SAFETY comments
- Documentation review
- `CHANGELOG.md` entry, release tag

---

## Post v0.1.0

### v0.1.x — Stabilization / Audit / CI Foundation (in progress)

The v0.1.x release line freezes the v0.1.0 prototype, documents its
limitations, and adds the audit + CI foundation needed before
v0.2 modifies security boundaries. It adds no new OS functionality.

See [`docs/src/roadmap/v0.1.x-stabilization.md`](docs/src/roadmap/v0.1.x-stabilization.md)
and RFCs 024–030, 044–047 (`rfcs/`).

| Version  | Theme                                       | RFCs landed       |
|----------|---------------------------------------------|-------------------|
| v0.1.1   | Release freeze + CI foundation              | 024, 025          |
| v0.1.2   | Negative tests + threat model + ABI         | 026, 027, 028     |
| v0.1.3   | Capability / Lease / MMIO / DMA / Evidence  | 029, 030, 044     |
| v0.1.4   | ADR sync + release checklist                | 045, 046          |
| v0.1.5   | v0.2 preparation backlog                    | 047               |

### v0.2.0 — Security Boundary Closure (in progress: v0.2.9 hardening, post-review)

The first post-v0.1.x hardening milestone. Turns Fjell OS from a
local verified prototype into a system whose core security
boundaries are uniformly enforced. See the v0.2 RFC set (RFCs
031–043) and [`docs/src/security/v0.1.0-threat-model.md`](docs/src/security/v0.1.0-threat-model.md) §14.

| Phase | Name                                        | RFC      | Status |
|-------|---------------------------------------------|----------|--------|
| 1     | Capability Enforcement Core                 | 031, 032 | ✓ |
| 2     | Lease Revocation Semantics                  | 033, 034 | ✓ |
| 3     | MMIO Boundary Closure                       | 035      | ✓ |
| 4     | DMA Boundary Closure                        | 036      | ✓ |
| 5     | Cooperative Service Separation              | 037, 038 | ✓ |
| 6     | User Copy and Audit Drain                   | 039      | ✓ |
| 7     | cap-broker Bootstrap and Policy Enforcement | 040      | ✓ |
| 8     | Persistent Evidence Hardening               | 041      | ✓ |
| 9     | Negative Test Completion + Release Gate     | 042, 043 | ✓ |

**v0.2.9-v0.2.14 hardening releases** (COMPLETE):

| Release | Scope |
|---------|-------|
| v0.2.9 | ABI / test-harness correction (this release) |
| v0.2.10 | Capability/syscall enforcement closure |
| v0.2.11 | MMIO/DMA/audit hardening |
| v0.2.12 | Service separation + release-gate close |

**`TEST:V02:PASS` earned at v0.2.14 close.**

### Beyond v0.2 — executed

All post-v0.2 release lines are complete through **v0.21.2** (the current
release). Each line delivered a coherent theme:

| Line | Theme | Status |
|---|---|---|
| v0.3.0 | Hardware Trust Abstraction | ✅ |
| v0.4.0 | Minimal Secure Networking | ✅ |
| v0.5.0 | Multi-Platform Foundation + Semantic API Stabilization | ✅ |
| v0.6.0 | Verification / Property Testing (original M10) | ✅ |
| v0.7.0 | Distributed Snapshot Sync Foundation | ✅ |
| v0.8.0 | Fleet / Edge Operations Plane | ✅ |
| v0.9.0 | Developer Service Platform (original M11) | ✅ |
| v0.10.0 | Release Maturity (reproducible build, ABI, gates) | ✅ |
| v0.11.0 | Trust Spine Hardening | ✅ |
| v0.12.0 | Deployment Profile Hardening | ✅ |
| v0.13.0 | Fleet Reliability and Recovery Depth | ✅ |
| v0.14.0 | Developer Ecosystem Trial | ✅ |
| v0.15.0 | v1.0 Freeze Candidate | ✅ |
| v0.16.0 | Ed25519 Interoperability Closure | ✅ |
| v0.17.0 | Trust Anchor Provisioning and Manufacturing Flow | ✅ |
| v0.18.0 | Verus Promotion to Release-Required | ✅ |
| v0.19.x | Negative-test conversion (found six latent kernel bugs) | ✅ |
| v0.20.x | v1-readiness: fail-closed gate, IPC ABI fix (E-010) | ✅ |
| v0.21.x | Crate reorganization, audits, handoff + design docs | ✅ |
| v0.21.3 | Build restoration, as-built reconciliation, v0 release cycle | ✅ released |

### v0.22 — Gate Integrity (planned; owner-approved 2026-07-30)

v0.21.3 found **four** separate instances of a mechanical gate reporting
green while a documented rule went unmet. That is one class of defect, not
four bugs. Every completion claim in this project is settled by the eleven
gates, so v0.22 makes them mean what they claim before further function is
built on top of them.

Governing principle for the line: **every gate added or strengthened must be
demonstrated failing on a deliberately broken input before it is accepted.**

| # | Item |
|---|---|
| 1 | Gate 11 from substring matching to a real function-body scan (architect review H-03) |
| 2 | Gate 4 ABI signature normalisation — today a whole-tree `cargo fmt` invalidates the baseline wholesale |
| 3 | Mechanical syscall-count check, to stop documented-surface drift recurring |
| 4 | Bind documented rules to gates where cheap (ACCEPTED errata ↔ limitations; RFC folder ↔ Status; handoff status inheritance) |

Governed by `RFC-v0.22-001`. Out of scope: negative-coverage completion, the
9 undispatched syscalls, build determinism, DMA unmap, and anything touching
kernel/ABI/crypto behaviour.

### v0.23 — ABDD Live Path (planned; owner-approved 2026-07-31)

The first line in several to add runtime behaviour rather than documentation or
tooling. Fjell's distinguishing claim — applications emit meaning, a proxy
renders it — is currently demonstrated only by unit tests: `proxy-text` holds
845 lines of working renderer behind an entry point that prints one line and
exits, and `semantic-stream` is the same shape.

v0.23 connects them. A real service emits an intent node, `semantic-stream`
routes it, `proxy-text` renders it, and the proxy's return leg issues a
capability-checked `ActionRequest` — proven by refusal, not only by success.
The path is gated by a fail-closed QEMU profile so it cannot rot.

Adds no kernel surface and no syscalls. Governed by `RFC-v0.23-001`.

Chosen from four measured directions (`docs/src/roadmap/v0.23-direction-options.md`)
because it is roughly an order of magnitude smaller than any alternative,
depends on nothing, and is the only one producing a claim the project cannot
currently make.

### Beyond v0.23 — under discussion, not yet decided

**v1.0 is explicitly not in view** (owner, 2026-07-30); v0 development
continues. The owner has directed that functional advancement, not only
stabilization, must precede any v1.0 consideration — the current state is
far from production readiness or demonstrable appeal.

The options paper was prepared and the first direction chosen (v0.23, above).
The remaining directions stay **undecided** and are re-opened when v0.23 closes:

- Make the service plane real — 17 of 29 services never receive IPC
- Make it operable by a human — **kernel work first**: no console input path
  exists at any layer, so this needs UART RX, an interrupt path, and a read
  syscall before any userland command set
- Make it run on metal — hardware bring-up, currently placed at v2+

Full analysis, with measurements and the dependency map, in
`docs/src/roadmap/v0.23-direction-options.md`.

- Make the semantic plane real (the ABDD live path and beyond)
- Make the service plane real (17 of 29 services currently never receive IPC)
- Make the system operable by a human (base userland, FR-SVC-006)
- Make it run on metal (hardware bring-up)

A measured finding that bears on that discussion: `proxy-text` contains 845
lines of working renderer that nothing calls — its service entry point
prints one line and exits. The same is true of `semantic-stream`. Fjell's
distinguishing demonstration is largely built and entirely unwired, so the
gap may be smaller than the current state suggests.

**v1.0.0 — First Supported Profile** is architect-conditionally-approved.
At v0.21.2, the workspace manifest was broken (`cargo metadata` failed to
parse), so the eleven mechanical gates could not actually run — Gate 9
(manual limitations sign-off by the owner) was not the only blocker at that
point, even though it was described as such. `rfcs/done/RFC-v0.21.3-001-build-restoration-and-as-built-reconciliation.md`
restores the build and re-verifies the mechanical gates; see
`rfcs/handoffs/RFC-v0.21.3-001-build-restoration-and-as-built-reconciliation/`
for current gate status. Gate 9 remains the only blocker that requires the
owner rather than a mechanical check. v1.0.0 must not be tagged, published,
or announced without explicit owner confirmation.

---

For the full roadmap — the original M0–M11 MVP plan, the complete execution
record, and the forward roadmap (v1.0, v1.1, v2+) — see
[`docs/src/roadmap/roadmap.md`](docs/src/roadmap/roadmap.md).
