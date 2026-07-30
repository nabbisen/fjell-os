# Fjell OS — Roadmap and Milestones

*The development roadmap for Fjell OS: the original MVP milestone plan (M0–M11),
the actual execution record from v0.1.0 through v0.21.2, and the forward roadmap
from v1.0 onward. Derived from the original architecture roadmap
(`Fjell-OS-アーキテクチャ詳細化_開発ロードマップ_v1`, 2026-05-04) and reconciled
against the RFC register and release history.*

## Roadmap philosophy

Fjell's roadmap builds **trustworthy thin vertical slices** rather than adding
features horizontally. It deliberately excludes many drivers, GUI, networking,
AI, and compatibility layers early. The ordering is fixed:

```text
1. Boot        →  it starts
2. Isolate     →  it separates
3. Communicate →  it talks (IPC)
4. Delegate    →  it passes authority (capabilities)
5. Audit       →  it records
6. Reproduce   →  it rebuilds from declarative config
7. Mean        →  it emits semantic streams
8. Upgrade     →  it updates safely
9. Verify      →  it proves its invariants
```

Keeping this order is what makes Fjell "an OS whose responsibilities were pared
down to meet modern demands", not "an over-stuffed next-generation OS".

---

# Part 1 — The original MVP plan (M0–M11)

The initial roadmap defined twelve milestones. Each has a purpose, deliverables,
and completion criteria. This is the plan **as designed**; Part 2 records what
was actually built.

### Milestone 0 — Project Foundation

*Fix the development premises so the project cannot diverge.*
Deliverables: repository structure, coding standard, unsafe policy, target
architecture decision, boot strategy, license, README/docs skeletons, ADR
template, roadmap, QEMU execution policy. Done when an empty kernel crate builds,
the workspace passes CI, and the QEMU and unsafe policies are documented.

### Milestone 1 — Minimal Boot Kernel

*Boot a minimal kernel and take control.*
Deliverables: bootable kernel image, serial output, panic handler, minimal
memory layout, timer-interrupt skeleton. Done when the kernel boots under QEMU,
prints a boot message to the serial console, halts with a reason on panic, and a
smoke boot test runs from CI.

### Milestone 2 — Memory and Task Isolation

*Establish the minimum isolation an OS needs.*
Deliverables: physical frame allocator, virtual memory manager, address-space
abstraction, user-task abstraction, kernel/user mode transition. Done when a
user task starts and can trap into the kernel, invalid memory access is
detected, and kernel memory is unreadable from a user task.

### Milestone 3 — IPC and Capability Minimum

*Establish Fjell's core: the minimal IPC + capability model.*
Deliverables: endpoint capability, send/receive/call/reply, fixed-size IPC
message, capability table, simple delegation. Done when only a task holding the
capability can send to an endpoint, a task without it is refused, IPC call/reply
works between two user tasks, and authority violations can be recorded as audit
events.

### Milestone 4 — User-Space Init and Service Manager

*Shift from a kernel to a user-space-service-centric OS.*
Deliverables: init service, service manifest format, service-manager prototype,
service lifecycle, service status export. Done when init starts as the first
user-space service, the manager starts several services, service state is
queryable, and a crash is detected.

### Milestone 5 — Audit and Append-Only State

*Establish operational transparency.*
Deliverables: audit event format, audit service, append-only state record, event
sequence, plain-text export. Done when boot/service/capability events are
recorded, the audit log exports (e.g. JSON Lines), a `sequence` +
`previous_hash` tamper-evidence foundation exists, and the audit service
separates its read API from its write path.

### Milestone 6 — Declarative Configuration

*Reproduce system state from configuration files.*
Deliverables: TOML config schema, config service, validation, dry-run, apply
event, rollback metadata. Done when service configuration loads from TOML,
invalid configuration is rejected before apply, dry-run output is produced, and
config application is recorded in the audit log.

### Milestone 7 — Device Manager and Minimal Drivers

*Validate the user-space driver model.*
Deliverables: device manager, serial driver service, block-device mock or
virtio-block, driver restart policy, device state export. Done when a driver
runs as a user-space service, a device capability is passed to it, a driver
crash does not cascade into a kernel crash, and driver restart is audited.

### Milestone 8 — Semantic Stream and Text Proxy

*Implement ABDD as architecture, minimally.*
Deliverables: semantic-stream schema, IntentNode format, semantic-stream
service, text presentation proxy, operation-request flow. Done when a service
emits an Intent Stream, the text proxy displays it, the proxy can return an
operation request, that request is capability-checked, and basic operation can
be explained without any GUI dependency.

### Milestone 9 — Immutable Upgrade Prototype

*Build the foundation for long-lived operation and safe updates.*
Deliverables: image manifest, upgrade service, inactive-slot write, integrity
check, rollback flag. Done when an update never overwrites the running image in
place, a new image is verified before switching, a failed-boot rollback design
exists, and upgrade events are audited.

### Milestone 10 — Verification and Hardening

*Make Fjell's verifiability concrete.*
Deliverables: formal-model candidates, property tests, fuzz tests, syscall
tests, IPC/capability invariant tests, ADRs. Done when the major
capability/IPC invariants are tested or modelled, unsafe sites carry safety
comments, fuzz targets are defined, and developers can tell verified from
non-verified areas.

### Milestone 11 — Developer SDK and Documentation

*Make the developer experience of building services on Fjell workable.*
Deliverables: service SDK, IPC client library, capability/audit/semantic-stream
helpers, service template, developer guide. Done when a sample service can be
built with the SDK, that service uses IPC/audit/config/semantic-stream, and a
developer can implement a minimal service from the docs.

---

# Part 2 — Execution to date (v0.1.0 → v0.21.2)

The project executed the M0–M11 plan as **v0.1.0**, then continued well beyond
the original MVP with a disciplined series of hardening and capability-deepening
release lines. All 154 governing RFCs are resolved (`rfcs/done/`).

## v0.1.0 — Initial Release (M0–M8 delivered)

The original twelve milestones were realized as the v0.1.0 release, mapped to
the internal M-series design documents (`fjell-os-v0_1_0-mN-内部設計書`):

| Milestone | Delivered | Evidence |
|---|---|---|
| M0 Repository Foundation | ✅ | Cargo workspace, `no_std` kernel, docs/ADR skeleton, CI, LICENSE/NOTICE |
| M1 Bootable Kernel | ✅ | `link.ld` (QEMU virt @ `0x8000_0000`), `_start`, 16550A UART, boot banner, `cargo xtask qemu` |
| M2 Memory & Task Isolation | ✅ | M→S handoff, DTB memory discovery, `BootAllocator` + bitmap `FrameAllocator`, Sv39 tables, scheduler, `sys_yield`/`sys_exit`, fault containment; `TEST:M2:PASS` |
| M3 IPC & Capability | ✅ | rendezvous `Endpoint`, `Capability`/`CapRights`/`CapHandle`, derivation tree, `cap_copy/mint/delete/revoke`, `ipc_send/recv/call/reply`, audit hooks; `TEST:M3:PASS` |
| M4 init / service-manager | ✅ | `fjell-init`, `fjell-service-manager` with TOML manifest, sample-service lifecycle |
| M5 Audit & State Export | ✅ | `AuditEvent` ring → `fjell-auditd`, JSON Lines export, `previous_hash` chain |
| M6 Declarative Configuration | ✅ | TOML config schema + validation, dry-run/apply/rollback metadata |
| M7 Semantic Stream & Text Proxy | ✅ | `IntentNode` schema, `fjell-proxy-text`, sample-service emits intent |
| M8 v0.1.0 Hardening | ✅ | property tests (cap/IPC/scheduler), full unsafe audit, doc review, release tag |

> Note on numbering: the original plan's M9 (upgrade), M10 (verification), and
> M11 (SDK) were not dropped — they were deferred past the v0.1.0 prototype and
> delivered in later release lines (v0.9 upgrade path, v0.6/v0.18 verification,
> v0.9 SDK), where they could be built on a hardened security boundary.

## Post-v0.1.0 release lines

After freezing the prototype, the project added the hardening and depth the
original MVP intentionally left out. Each line is a coherent theme:

| Line | Theme | Status |
|---|---|---|
| **v0.1.x** | Stabilization / audit / CI foundation (freeze prototype, negative tests, threat model, ABI snapshot, lease/MMIO/DMA/evidence) | ✅ complete |
| **v0.2.x** | Security Boundary Closure — uniform enforcement of the core security boundaries (capability, lease revocation, MMIO, DMA, service separation, user-copy, cap-broker bootstrap, evidence, negative-test gate) | ✅ `TEST:V02:PASS` |
| **v0.3.0** | Hardware Trust Abstraction — `HardwareTrustProvider` interface + provider registry, keyring | ✅ complete |
| **v0.4.0** | Minimal Secure Networking — virtio-net user-space driver, network device capabilities, secure transport | ✅ complete |
| **v0.5.0** | Multi-Platform Foundation + Semantic API Stabilization — `PlatformProfile`/`BoardProfile`, frozen semantic catalog v1 | ✅ complete |
| **v0.6.0** | Verification / Property Testing — capability/IPC/lease property-test harness (realizes original M10) | ✅ complete |
| **v0.7.0** | Distributed Snapshot Sync Foundation — node identity, snapshot exchange trust model, offline-first sync | ✅ complete |
| **v0.8.0** | Fleet / Edge Operations Plane — fleet identity, enrollment, node registry, fleet policy | ✅ complete |
| **v0.9.0** | Developer Service Platform — service SDK + stable service API subset (realizes original M11) | ✅ complete |
| **v0.10.0** | Release Maturity — reproducible build, ABI stability, release-rehearsal gates | ✅ complete |
| **v0.11.0** | Trust Spine Hardening | ✅ complete |
| **v0.12.0** | Deployment Profile Hardening — first real-board profile groundwork | ✅ complete |
| **v0.13.0** | Fleet Reliability and Recovery Depth | ✅ complete |
| **v0.14.0** | Developer Ecosystem Trial | ✅ complete |
| **v0.15.0** | v1.0 Freeze Candidate Overview | ✅ complete |
| **v0.16.0** | Ed25519 Interoperability Closure | ✅ complete |
| **v0.17.0** | Trust Anchor Provisioning and Manufacturing Flow | ✅ complete |
| **v0.18.0** | Verus Target Promotion to Release-Required (capability + lease proofs now block a release) | ✅ complete |
| **v0.19.x** | Negative-test conversion — nine QEMU negative categories made real (found six latent kernel bugs) | ✅ complete |
| **v0.20.x** | v1-readiness — fail-closed negative-test gate; IPC words ABI fix (E-010); reply-edge cancellation; architect review corrections | ✅ complete |
| **v0.21.x** | Structural — crate subdirectory reorganization, horizontal file cleanup, five-dimension audit, handoff bundle, requirements + external-design docs | ✅ current (v0.21.2) |

## Current state (v0.21.2)

- **80 crates**, reorganized into `arch/`, `drivers/`, `formats/`, `services/`
  plus flat library/infra crates.
- **Correction (RFC-v0.21.3-001):** at v0.21.2 the workspace manifest was
  broken (`cargo metadata` failed to parse), so the eleven mechanical release
  gates could not actually run — the "all eleven pass" and "Gate 9 is the
  single remaining blocker" claims below described an intended, not a
  verified, state. `rfcs/proposed/RFC-v0.21.3-001-build-restoration-and-as-built-reconciliation.md`
  restores the build and re-runs the gates; see
  `rfcs/handoffs/RFC-v0.21.3-001-build-restoration-and-as-built-reconciliation/`
  for current, verified gate status.
- **v1.0.0 is architect-conditionally-approved.** Gate 9 — the manual
  limitations sign-off by the owner (nabbisen) — remains the only blocker
  that requires the owner rather than a mechanical check.
- Publication control: v1.0.0 must not be tagged, published, or announced
  without explicit owner confirmation.

---

# Part 3 — Forward roadmap (v1.0 and beyond)

## v1.0.0 — First Supported Profile

The first supported QEMU profile of Fjell OS. Gated on Gate 9 sign-off and
strict release wording. The approved claim scopes v1.0 as a narrow,
QEMU-supported, high-assurance prototype — not a production hardware OS. It
explicitly does **not** claim real-hardware readiness, full store/upgrade
negative coverage, complete service-manager lifecycle coverage, a production
trust-anchor lifecycle, a fully verified kernel, POSIX compatibility, or
general-purpose OS readiness.

**Remaining before the tag:** Gate 9 manual sign-off, plus re-verification
that the eleven mechanical gates actually pass post-RFC-v0.21.3-001 (see
"Current state" above) — the gates were unreachable at v0.21.2 and are being
re-run, not re-litigated, by that RFC.

## Immediately after v1.0.0 (required soon, per architect review)

These are not v1.0 blockers but are required shortly after:

| Item | Origin |
|---|---|
| Store/upgrade negative emitters (`NEG:STORE:*`, `NEG:UPGRADE:*`) | Architect review H-04 |
| IPC register-layout ABI snapshot/regression | Architect review H-01 (doc landed; regression pending) |
| Stronger callsite-audit (function-body scan, not heuristic) | Architect review H-03 |
| End-to-end provision/sign/verify dev workflow | Architect review §4.4 |
| Verus pilot proofs machine-checked + C4/C6/C7/C8 corrections applied | v0.17 carry-over |

## v1.1 — Hardening the deferred boundaries

| Theme | Requirement / limitation closed |
|---|---|
| Store & upgrade negative profiles become mandatory release gates | v1.0 limitation (deferred store/upgrade) |
| Service-manager READY negative pair completed (2/4 → 4/4) | v1.0 partial svc coverage |
| Factory-station trust-anchor provisioning | Requirements limitation item 6; RFC-v0.17-001 |
| End-to-end provision + sign + verify workflow gate | Signing-side coupling closure |
| DMA user-VA unmap re-enabled (root-cause the v0.8.x page-table corruption) | Kernel debt |

## v2 and beyond — Longer-horizon directions

These realize parts of the original requirements deferred as non-goals for the
initial phases (requirements §7.3, §8 "Could"):

| Direction | Notes |
|---|---|
| **Real hardware bring-up** | Boot on silicon (e.g. StarFive VisionFive 2); the provisional board profile becomes validated. Closes v1.0 limitation item 1 (E-004). |
| **Multi-hart / SMP** | SMP scheduling, per-hart locking, IPIs. Closes v1.0 limitation item 2. |
| **Hardware-anchored provisioning** | Trust anchor rooted in hardware (PMP/enclave), superseding dev/QEMU TOFU. |
| **Active power-state scheduling** | Realize FR-KRN-006 fully: suspend/resume coupled to hardware C-states, beyond today's telemetry. |
| **Personal Proxy & continuous state measurement** | The ABDD analysis's second and third shifts: proxy-side dynamic adaptation and continuous telemetry-driven adjustment (beyond the text reference proxy). |
| **Richer Presentation Proxies** | Audio, braille, and other proxies on the existing semantic boundary. |
| **Multi-architecture** | Bring `fjell-arch-arm64` from stub to a real target. |

## Explicit non-goals (unchanged across the roadmap)

Per the requirements' "will not do" declarations, Fjell does **not** pursue:
becoming a general-purpose desktop OS; full POSIX/Linux compatibility; a GUI
stack in the OS core; an AI-native kernel; a `root`-premised authority model;
pulling drivers back into the kernel; cloud-mandatory operation; or a universal
accessibility-settings collection.

## Success criteria (the roadmap's north star)

Success was never defined as "a convenient OS". It is defined as:

```text
- it boots
- it isolates
- it controls authority via capabilities
- it communicates safely via IPC
- it audits its state
- it reproduces state declaratively from config
- it emits meaning without a GUI
- the OS core stays small
```

Holding the fixed ordering above is what keeps Fjell on that line.
