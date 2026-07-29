# Fjell OS — Requirements Definition

This is the authoritative requirements
baseline; functional and non-functional requirements retain their original
identifiers (FR-*, NFR-*) so they can be cross-referenced from RFCs and code.*

## 1. Project overview

### 1.1 Project name

**Fjell OS**

Fjell aims to be a **small, verifiable, memory-safe, and sustainable operating
system** for modern industrial, edge, and long-lived deployment environments.

### 1.2 Purpose

Fjell OS exists to provide a new foundation that narrows the responsibilities of
the OS, addressing these root problems of existing general-purpose systems:

1. Reduce vulnerabilities that stem from memory unsafety and implicit trust
   boundaries.
2. Bring formal verification — impractical for bloated monolithic systems —
   within a realistic scope.
3. Separate drivers, file systems, and audit functions so that the blast radius
   of a fault is bounded.
4. Deliver low power consumption and safe updates for long-lived industrial and
   edge devices.
5. Separate GUI rendering responsibility from the OS core, so that meaning,
   state, and intent can flow safely — making ABDD an architectural property
   rather than a bolt-on feature.

### 1.3 Core value proposition

> **Fjell OS is a memory-safe OS for industrial and edge use that, through a
> verifiable minimal core, safely separates and connects processes,
> authority, state, and semantic streams.**

## 2. Design principles

### 2.1 Memory-Safe by Design

The kernel, userland, and base services are implemented in Rust or an
equivalently memory-safe technology. The goal is to suppress typical memory
safety problems — buffer overflow, use-after-free, data races — at the design
and compile stage rather than through runtime mitigation.

### 2.2 Verifiable Core

Formal verification is not spread too widely; it is initially limited to the
minimal area directly tied to OS safety. The initial verification targets are:

- Process isolation
- Memory authority
- IPC safety
- Capability creation, delegation, and revocation
- Basic scheduler invariants
- The boot-time trust boundary

### 2.3 Microkernel & User-Space Services

The kernel is limited to: address-space separation, thread/task management,
IPC, capability management, interrupt management, and a minimal hardware
abstraction. Device drivers, file systems, the network stack, the audit
service, and the configuration-management service are, as a rule, implemented
as user-space services.

### 2.4 Capability-Based Security

Fjell OS does not place a traditional `root`-centered authority model at its
core. Each process may access memory, devices, ports, files, and state APIs
only within the scope of capabilities explicitly passed to it at startup or via
IPC.

### 2.5 Minimalist Unix Reinterpretation

Fjell reinterprets the Unix philosophy of "do one thing well" for modern OS
design. But "everything" in Fjell is not merely a file — it is three kinds of
stream:

- **State Stream** — system state
- **Event Stream** — audit, fault, and operational events
- **Intent Stream** — the meaning and intent a service presents outward

### 2.6 ABDD as Architecture

ABDD (Accessible by Default and by Design) is not a collection of bolt-on
features like screen readers or color correction. In Fjell, applications do not
draw pixels; they emit meaning, state, options, and operational intent as
structured data. Rendering, speech, braille, simplified display, and personal
optimization are handled not by the OS core but by an external or user-space
**Personal Proxy / Presentation Proxy**. This design avoids the OS trying to
enumerate every accessibility pattern, and lets unknown needs be met on the
transformation side of the semantic stream.

## 3. Intended domains

### 3.1 Primary

- Industrial edge devices
- Auxiliary control nodes for factories and facilities
- Embedded devices requiring long-lived operation
- Gateways where security and auditability matter
- Systems without a display, or where the UI is auxiliary
- High-assurance local processing platforms
- Devices that need accessible external-UI integration

### 3.2 Secondary

- Research OS
- Educational platform for secure OS architecture
- Experimental platform for capability OSes
- Verification-experiment platform for Rust-based OSes
- Foundational research platform for ABDD-style UI/UX

## 4. "Will not do" declarations

To keep the purpose clear, Fjell OS explicitly will **not** do the following.

**4.1 Not aim to be a general-purpose desktop OS** — no full desktop
environment, consumer app store, gaming, high-end GUI workstation use (video
editing, 3D), or drop-in compatibility with existing desktop apps.

**4.2 Not aim for full Linux/POSIX compatibility** — a compatibility layer may
be considered later, but it is not a core value, and compatibility must never
complicate the kernel or the authority model.

**4.3 Not adopt a monolithic kernel** — drivers, file systems, and the network
stack are not pulled into the kernel for performance reasons alone; separation,
verifiability, and fault isolation take priority.

**4.4 Not center a traditional `root`-based authority model** — administrative
operations are also capability-based and must satisfy least privilege,
delegability, revocability, and auditability.

**4.5 Not include a GUI rendering stack in the OS core** — no X11/Wayland-scale
stack, font rendering, window manager, or theme engine in the core. Fjell
provides a standard stream of meaning, state, and operational intent, not
drawing commands.

**4.6 Not aim to enumerate individual accessibility patterns** — no fixed
categories ("for the visually impaired", "for the elderly") handled by
branching in the OS. Fjell provides the semantic stream and safe boundary that
*make* individual optimization possible, not the optimization itself.

**4.7 Not make an AI-native OS an initial goal** — AI inference, LLM agents, and
NPU scheduling are important future areas but not the initial core (memory
safety, a verifiable minimal core, capability security, sustainable long-lived
operation, and separation of display from processing via semantic streams). AI
may later connect as a Personal Proxy or operational aid, but must not become a
source of kernel complexity.

**4.8 Not be a cloud-dependent OS** — boot, authentication, audit, configuration
application, and basic operation do not require a cloud connection. Cloud
integration is an optional external service.

**4.9 Not aim to support all hardware initially** — the initial set of
architectures is limited; verifiability, isolation, power control, and boot
reproducibility take priority over broad device coverage.

**4.10 Not be a giant integrated auth/audit/config product** — Fjell does not
try to encompass SIEM, MDM, IAM, EDR, and configuration-management SaaS. The OS
emits trustworthy state and events and provides the boundary at which external
tools consume them safely.

## 5. Functional requirements

### 5.1 Kernel

**FR-KRN-001: Minimal microkernel.** Fjell adopts a microkernel structure. The
kernel is limited to CPU initialization, minimal memory management,
address-space separation, task management, scheduling, IPC, capability
management, interrupt management, minimal device abstraction, and starting the
initial services at boot.

**FR-KRN-002: Address-space separation.** Each process/service runs in an
independent address space. Accessing another process's memory requires an
explicit capability and a sharing procedure over IPC.

**FR-KRN-003: Capability management.** The kernel manages unforgeable
capabilities. A capability can express: memory-region access, IPC-endpoint
send/receive rights, device-operation rights, file/state-store access, audit-API
read rights, and service start/stop/restart rights. Capabilities support
delegation, restricted delegation, revocation, and audit recording.

**FR-KRN-004: IPC.** The kernel provides safe IPC that satisfies:
capability-checked send/receive authority, clear message boundaries, support for
typed messages, room for zero-copy or low-copy communication, and a structure
that makes deadlocks and authority confusion easy to verify.

**FR-KRN-005: Scheduling.** The kernel provides a lightweight, predictable
scheduler that handles basic priority, service class, power state, interrupt
response, long idle states, and an auditable execution history. Advanced
semantic scheduling for AI inference is out of scope initially.

**FR-KRN-006: Power-state coupling.** The kernel supports task suspend/resume
that accounts for hardware low-power states: CPU idle, device suspend, stopping
inactive services, wake-up events, and power-related audit events.

**FR-KRN-007: Use of hardware protection.** Memory protection, secure boot, PMP,
IOMMU, and enclave-equivalent mechanisms provided by the target hardware are
integrated into Fjell's security model — while avoiding excessive dependence on
specific vendor features.

### 5.2 Boot and image management

**FR-BOOT-001: Verifiable boot.** Fjell can verify the integrity of the kernel,
initial services, and configuration files loaded at boot.

**FR-BOOT-002: Minimal initial services.** The services started at boot are
limited to an explicitly defined minimal set. Candidates: init service,
capability broker, configuration service, audit service, state-store service,
device manager, and a basic console or semantic-output service.

**FR-BOOT-003: Atomic upgrade.** An OS update does not overwrite the running
system in place. A new image is placed in a separate region, verified, and the
boot target is then switched. On failure, rollback to the previous version must
be possible.

### 5.3 User-space services

**FR-SVC-001: User-space drivers.** Device drivers run as user-space services.
On driver failure, only that driver service — not the whole OS — can be stopped
and restarted.

**FR-SVC-002: Device manager.** Performs device discovery, capability
assignment, driver startup, and fault detection.

**FR-SVC-003: File-system service.** Provided as a user-space service handling
at least: a read-only system region, an append-only state region, a temporary
region, a configuration-file region, and an audit-log region.

**FR-SVC-004: Append-only state store.** Audit logs, critical state, and
configuration-change history are kept in an append-only store that is
tamper-evident, preserves temporal ordering, is usable for rollback or
reconstruction, and can export to a human-readable form.

**FR-SVC-005: Configuration-management service.** OS configuration is managed in
a declarative text format (initial candidate: TOML). The service performs
loading, schema validation, diff detection, pre-apply validation, apply-history
recording, and rollback support.

**FR-SVC-006: Single-binary base userland.** Base commands are provided as a
single (or few) statically-linked binaries with minimal dependencies. Initial
candidates: list, read, write, copy, move, remove, inspect, a grep equivalent,
state export, service status, capability inspect, audit read, config validate.
Whether to adopt Unix-compatible names verbatim is decided in the next phase.

### 5.4 Audit and observability

**FR-AUD-001: Continuous audit API.** A read-only audit API for safely reading
current system state: running services, process list, capability list, memory
usage, IPC connections, device state, configuration state, power state, boot
image info, and audit logs.

**FR-AUD-002: Plain-text state export.** System state can be exported in a
human-readable form for incident investigation and periodic audit. Initial
candidate formats: TOML, JSON Lines, CSV, Markdown summary.

**FR-AUD-003: Authority-delegation audit.** Capability creation, delegation,
restriction, and revocation are recorded as audit-log entries.

**FR-AUD-004: Configuration-change audit.** Configuration-file change,
validation, application, failure, and rollback are traceable.

**FR-AUD-005: Fault-event audit.** Service crashes, restarts, IPC errors,
authority denials, and device anomalies are recorded as standardized events.

### 5.5 ABDD / semantic-stream requirements

**FR-SEM-001: Intent Stream.** Applications and services can emit, toward an
external interface, meaning/state/operational-intent rather than drawing
commands. The Intent Stream can express: state display, options, confirmation
requests, warnings, input requests, progress, errors, recommended operations,
reasons an operation is unavailable, and required authority.

**FR-SEM-002: Presentation Proxy boundary.** Fjell provides a boundary that
safely connects to a Presentation Proxy which receives the Intent Stream and
converts it into a visual, audio, braille, simplified, or machine-processing UI.
The OS does not prescribe the proxy's concrete presentation method.

**FR-SEM-003: Personal Proxy support.** Optimization for each user's physical
traits, cognitive traits, operating environment, and fatigue is handled by the
Personal Proxy, not the OS core. Fjell provides the structured meaning, state,
and operability the Personal Proxy needs.

**FR-SEM-004: Authentication and integrity of the semantic streams.** The
Intent/State/Event streams flowing between the OS and a proxy must be
tamper-evident. At least the following are considered: per-session capability,
message signing, monotonically increasing sequence numbers, replay prevention,
read-only channels, and per-proxy authority scope.

**FR-SEM-005: UI-independent operation.** Basic OS operations must not depend on
a specific GUI. The same operation must be executable from at least: a text
console, a structured API, audit/config tooling, a Presentation Proxy, and
automated-operations tooling.

### 5.6 Security

**FR-SEC-001: Least-privilege execution.** Every service receives only the
minimal capabilities it needs at startup.

**FR-SEC-002: Explicit delegation of authority.** Authority delegation between
services must not be implicit; the source, target, subject capability,
restrictions, expiry, and revocation conditions are recorded.

**FR-SEC-003: Sandboxing.** User-space services are restricted in access scope
by capabilities. Driver, file-system, and network services each run in
independent sandboxes.

**FR-SEC-004: Secure failure.** On failure of authority validation,
configuration validation, boot validation, or update validation, Fjell fails
safe — meaning, as a rule: grant no authority, start no service, apply no
change, revert to the previous version, and leave an audit-log record.

### 5.7 Developer-facing

**FR-DEV-001: Service-development SDK.** Fjell provides an SDK for developing
user-space services, including: an IPC library, a capability-receipt API, a
configuration-read API, an audit-event-emit API, an Intent-Stream-emit API,
error types, and test-support tooling.

**FR-DEV-002: Semantic-stream schema.** The Intent/State/Event stream schemas
are published; they must be human-readable, machine-verifiable, and versionable.

**FR-DEV-003: Verifiable interface definitions.** The IPC, capability,
configuration, and audit-event interfaces are defined with a clear IDL or schema
usable for formal or static verification.

## 6. Non-functional requirements

### 6.1 Security

- **NFR-SEC-001: Default deny.** Undefined access, undelegated capabilities, and
  unvalidated configuration are all rejected.
- **NFR-SEC-002: Minimize attack surface.** Minimize the code and dependencies
  of the kernel, initial services, and base userland. In particular, the kernel
  must not contain a complex file system, a network protocol stack, a GUI stack,
  font rendering, a dynamic plugin system, or an advanced app-compatibility
  layer.
- **NFR-SEC-003: Tamper detection.** The boot image, configuration, audit logs,
  and state store must be tamper-evident.
- **NFR-SEC-004: Auditability.** Important security events must be traceable
  after the fact.

### 6.2 Reliability and availability

- **NFR-REL-001: Fault isolation.** A fault in a driver, file system, network
  service, or proxy must not directly halt the whole kernel.
- **NFR-REL-002: Restartability.** User-space services must, where possible, be
  individually restartable.
- **NFR-REL-003: Update resilience.** A power loss or validation failure during
  an update must not render the system unbootable.
- **NFR-REL-004: Long-lived operation.** Fjell assumes long, unattended
  operation; configuration, state, audit logs, and the update mechanism are
  designed to resist degradation, bloat, and dependence on individuals over
  time.

### 6.3 Verifiability

- **NFR-VER-001: Bounded verification target.** Formal verification is not
  applied without limit; initially it is bounded to kernel-safety invariants.
- **NFR-VER-002: Spec-to-implementation correspondence.** Verified modules make
  the correspondence among spec, implementation, tests, and verification
  conditions explicit.
- **NFR-VER-003: Simple state transitions.** The state transitions of
  capability, IPC, scheduling, and boot must be simple and explicit, to keep
  them verifiable.

### 6.4 Maintainability

- **NFR-MNT-001: Small boundaries.** Each component has a clear responsibility
  and a small API boundary.
- **NFR-MNT-002: Declarative configuration.** Configuration must be reproducible
  as declarative text, not hidden state or GUI-operation history.
- **NFR-MNT-003: Constrained dependencies.** The kernel, init, audit,
  configuration management, and base userland in particular have strictly
  limited dependencies.
- **NFR-MNT-004: Documentation-first.** Design decisions, boundaries,
  constraints, and non-goals are documented for developers, operators, auditors,
  and researchers.

### 6.5 Performance

- **NFR-PERF-001: Low-overhead IPC.** Keep the IPC overhead of the microkernel
  structure low — without sacrificing isolation or verifiability for speed.
- **NFR-PERF-002: Predictability.** Prioritize suppressing abnormal latency and
  unpredictable behavior over average performance.
- **NFR-PERF-003: Lightweight boot.** Minimize the components loaded at boot to
  keep boot time and memory usage low.

### 6.6 Power efficiency and sustainability

- **NFR-GRN-001: Power as a first-class requirement.** Power efficiency is a
  basic requirement of scheduling, service management, and audit — not a
  bolt-on optimization.
- **NFR-GRN-002: Power-state visibility.** The power state of each process,
  service, and device is observable.
- **NFR-GRN-003: Stopping unneeded services.** Unused or idle services can be
  safely stopped or suspended.

### 6.7 ABDD / accessibility

- **NFR-ACC-001: No loss of meaning.** When an application/service presents
  state or options externally, it must include not just a display string but the
  meaning, operability, importance, danger level, and input constraints.
- **NFR-ACC-002: Display-method independence.** Basic OS operations must not
  depend on a visual GUI.
- **NFR-ACC-003: Extensibility for unknown needs.** The OS does not fix user
  categories; transformation rules are extended on the proxy side.
- **NFR-ACC-004: Human readability.** Audit, configuration, and state output
  favor formats understandable without specialized tools.

### 6.8 Compatibility

- **NFR-COMP-001: Design purity over compatibility.** Compatibility with
  existing OSes matters, but is not pursued at the cost of Fjell's core design.
- **NFR-COMP-002: Absorb external integration at the boundary.** Integration
  with existing Linux, cloud, audit tools, AI tools, and UI tools is absorbed in
  user-space services or proxies, not inside the kernel.

## 7. Initial MVP scope

### 7.1 Purpose of the MVP

The MVP does not implement the whole Fjell vision. Its purpose is to test one
hypothesis:

> By combining a minimal microkernel, capabilities, user-space services,
> append-only audit, and semantic streams, can we compose a verifiable, small,
> operable OS foundation?

### 7.2 Included in the MVP

Minimal boot; address-space separation; a basic scheduler; capability
management; IPC; init service; audit service; configuration service; state
export; a sample user-space service; a sample Intent Stream; a simple
Presentation Proxy; basic static or specification checking; minimal
documentation.

### 7.3 Not included in the MVP

Full GUI; many device drivers; a Linux compatibility layer; an advanced network
stack; an AI-inference platform; distributed sync; a commercial-grade
secure-boot CA design; complete formal verification; a general-purpose
application environment.

## 8. Requirement priorities

**Must:** memory-safe implementation policy; minimal microkernel;
capability-based authority; IPC; user-space service separation; append-only
audit; declarative configuration; plain-text state export; atomic-upgrade
policy; GUI-independent semantic-stream design; maintaining the "will not do"
list.

**Should:** verifiable specification separation; low-overhead IPC; power-state
coupled scheduling; a sample Presentation Proxy; configuration-schema
validation; a standard schema for audit events; a driver-restart mechanism.

**Could:** multiple-architecture support; advanced power visualization;
external-audit-tool integration; AI-assisted operations; a Linux compatibility
sandbox; an advanced Personal Proxy.

**Won't (initial phase):** becoming a general-purpose desktop OS; full POSIX
compatibility; a large GUI environment; an AI-native kernel; cloud-mandatory
operation; a universal accessibility-settings collection.

## 9. Acceptance criteria

### 9.1 Architecture

- Kernel responsibilities are clearly bounded.
- Drivers, file systems, audit, and configuration management are separated
  outside the kernel.
- Authority is explainable on a capability basis rather than a `root` premise.
- The OS boundary is explainable as a semantic stream rather than GUI rendering.

### 9.2 Security

- Resources cannot be accessed with an undelegated capability.
- Capability creation, delegation, and revocation are auditable.
- A service fault does not directly halt the kernel.
- Configuration changes and updates are verifiable.

### 9.3 Operations

- Current state can be exported in a human-readable form.
- Major fault events are traceable from the audit log.
- System state can be reproduced from configuration files.
- An update failure can be safely reverted.

### 9.4 ABDD

- Basic operations do not depend on a specific GUI.
- Services can emit state, options, and operational intent in structured form.
- A Presentation Proxy can receive the Intent Stream and convert it to another
  form.
- Accessibility is not implemented as branching over fixed categories.

## 10. Items to be detailed in the next development directive

1. Target CPU architecture (e.g. RISC-V first, experimental x86_64).
2. Rust implementation policy (`no_std`, unsafe policy, crate split, test
   policy).
3. Kernel internal structure (scheduler, memory, ipc, capability, interrupt,
   boot).
4. Capability model (data structures, lifecycle, delegation, revocation,
   audit).
5. IPC IDL (typed messages, errors, authority checks).
6. Audit-log format (event kinds, timestamps, signatures, append-only store).
7. Configuration-file format (TOML schema, validation, apply procedure).
8. Intent-Stream schema (state, options, input, danger level, operability).
9. Presentation-Proxy samples (text UI, audio UI, web-like rendering).
10. Verification strategy (formal verification, model checking, property
    testing, fuzzing).
11. MVP roadmap (split roughly into Milestone 0 to Milestone 3).

## Note

Fjell OS is best defined not merely as "an OS written in Rust", but as a project
that **shrinks the responsibility of the OS itself and keeps only the verifiable
boundaries at its core**. The crucial point is treating ABDD not as an
"additional accessibility feature" but as **a design principle for separating
display from processing so the OS can carry semantic streams safely**. This
makes formal verification, minimal Unix, sustainability, and inclusion all point
in the same direction.
