# External Design — Developer Surface

*Subsystem 9 of 9. Anchored to FR-DEV-001…003, NFR-MNT-* and `fjell-sdk`,
`fjell-semantic-v1`, `fjell-service-api`, the ABI snapshot at v0.21.2.*

## 1. Responsibility

This subsystem is the contract Fjell offers to people building on it: an SDK for
writing user-space services, published semantic-stream schemas, and interface
definitions clear enough for formal or static verification. It is the surface
that keeps the system maintainable and extensible (NFR-MNT-*).

## 2. External surface

### Service-development SDK (FR-DEV-001, as-built `fjell-sdk`)

The SDK provides what a service author needs: an IPC library
(`fjell-syscall` wrappers), a capability-receipt API, a configuration-read API,
an audit-event-emit API, an Intent-Stream-emit API, error types, and
test-support tooling. The reference service is `sample-service`; the authoring
guide is [Writing a Service](../sdk/writing-a-service.md).

### Semantic-stream schema (FR-DEV-002, as-built `fjell-semantic-v1`)

The Intent/State/Event schemas are published, human-readable, machine-verifiable,
and versioned. The v1 catalog is frozen (ADR-v0.5-004) and auto-published to
[Intent Catalog v1](../api/semantic-catalog.md).

### Verifiable interface definitions (FR-DEV-003, as-built)

The IPC, capability, configuration, and audit-event interfaces are defined as
typed Rust in dedicated format crates (`fjell-*-format`) and the ABI surface
(`fjell-abi`), usable for static verification. The ABI is snapshot-gated (401
tracked items at v0.21.2); removals fail the release gate.

## 3. Requirements coverage

| Requirement | Design response | As-built evidence |
|---|---|---|
| FR-DEV-001 SDK | IPC + cap-receipt + config-read + audit-emit + intent-emit + errors + test tools | `fjell-sdk`, `sample-service` |
| FR-DEV-002 Semantic schema | Published, versioned, machine-verifiable | `fjell-semantic-v1`, semantic-catalog |
| FR-DEV-003 Verifiable interfaces | Typed format crates + ABI snapshot | `fjell-*-format`, `fjell-abi` |
| NFR-MNT-001 Small boundaries | Each service/crate has a small, clear API | crate-per-responsibility layout |
| NFR-MNT-002 Declarative config | TOML manifests, reproducible | `fjell-config-format` |
| NFR-MNT-003 Constrained dependencies | Kernel/init/audit/config/userland deps strictly limited | workspace dependency policy |
| NFR-MNT-004 Documentation-first | Design/boundaries/constraints/non-goals documented | this docs tree; RFC register |

## 4. Stability contract

The ABI surface is frozen from v1.0 and governed by the snapshot gate: any
removal fails CI. Changes require an RFC with an architect decision record (see
[ABI Stability Policy](../abi/policy.md)). The IPC register layout
([IPC Register Layout](../abi/ipc-register-layout.md)) is normative. This gives
downstream service authors a stable base to build against.

## 5. Development discipline (from the project's own rules)

- Rust 2024, `no_std`, no `mod.rs` (a `foo.rs` + `foo/` subdir coexist).
- Test modules separated from implementation (`src/x.rs` ↔ `src/x/tests.rs`).
- File-splitting guidance at 300/500 effective lines.
- Design-before-code: Requirement → External Design → Internal Design →
  Program Design → Implementation → Testing.
- Every significant change is governed by an RFC (`rfcs/`, lifecycle policy in
  `000-rfc-lifecycle-policy.md`); ERRATA.md is the drift register.

## 6. As-built scope limits & gaps

- **The SDK reference service does not use live kernel-mediated IPC** at v1.0
  (non-goal N21); it demonstrates the authoring pattern rather than a full live
  IPC round-trip.
- The base userland command set (FR-SVC-006) is specified but not the v1.0
  focus; the SDK surface to build such commands exists.

## 7. Related subsystems

The developer surface exposes contracts from every other subsystem:
[IPC](./ipc.md), [Capability & Lease](./capability-lease.md),
[Audit & Observability](./audit-observability.md), and
[ABDD / Semantic Streams](./abdd-semantic.md).
