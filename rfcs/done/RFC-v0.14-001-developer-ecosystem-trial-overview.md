# RFC-v0.14-001 — Developer Ecosystem Trial Overview

**Status:** Implemented (v0.14.0)
**Target version:** v0.14.0
**Parent:** RFC 061 §10.
**Cross-refs:** v0.14-002 through v0.14-005.

## 1. Purpose

v0.9 built the SDK. v0.10 documented it. v0.11–v0.13 hardened the
substrate the SDK runs against. Nobody has actually written a
non-trivial service *using* the SDK constraints. v0.14 finds out
whether the SDK is honest.

The single most important goal: deliver a real service, written
strictly within `fjell-sdk`'s stable surface, that exercises every
v0.9 mechanism end-to-end (CapManifest, bundle, dev-harness) and
documents every place the experience needed friction it shouldn't have.

The honesty check is structural: if the SDK author cannot build a
real service without reaching past the SDK boundary, the SDK is wrong
and v0.14 surfaces that before v1.0 freezes it.

## 2. Composition

| RFC | Title | Deliverable |
|-----|-------|-------------|
| v0.14-001 | This overview | Coordination |
| v0.14-002 | First Non-Trivial External Service (Reference) | One real service crate, exemplary |
| v0.14-003 | Typed Catalog Structs and Service Cookbook | Auto-generated typed emitters + recipes |
| v0.14-004 | Bundle Publishing Flow and Local Artifact Registry | `cargo xtask publish`, registry format |
| v0.14-005 | Developer Mode Tooling (`--trace`, `--measure`, `--gdb`) | Three new dev-harness modes |

## 3. The honesty mechanism

Two rules govern every v0.14 deliverable:

1. **No SDK escape hatches.** The reference service uses only
   `fjell-sdk` re-exports. If a feature requires reaching past the
   SDK boundary, the SDK is missing that feature; the gap is filed as
   an RFC update before v0.14 lands.
2. **Pain logged.** Every rough edge encountered while authoring the
   reference service is documented in
   `docs/sdk/lessons-from-v0.14.md`. Roughness with no resolution
   becomes either a follow-up RFC or a known-limitation entry in the
   v1.0 readiness matrix (RFC-v0.10-007).

These rules cost speed and produce truth.

## 4. Posture

v0.14 *is not* an ecosystem-recruitment milestone. There is no public
call for service authors, no marketplace, no marketing. v0.14 is an
internal trial. The goal is to confirm that an external author *could*
succeed, not to recruit one.

External authoring opens after v1.0 with a separate, deliberate plan.

## 5. What v0.14 explicitly does *not* include

- A public service marketplace or registry.
- Per-service signing keys held by external parties.
- A service-author onboarding programme.
- Multi-language SDK bindings (Rust only for v1.0).
- A package manager. The bundle publishing flow is a file-based
  registry, not Cargo-equivalent dependency resolution.
- Sandbox-only / unprivileged service authoring without CapManifest
  review.

## 6. Release criteria

v0.14.0 may be tagged when:

1. The four sub-RFCs (002–005) are merged to `done/`.
2. The reference service from v0.14-002 builds, signs, publishes,
   deploys, and runs correctly on the v0.10-005 reference fleet.
3. `docs/sdk/lessons-from-v0.14.md` exists with at least one
   actionable lessons-learned entry (an empty list would be
   suspicious).
4. The typed catalog structs from v0.14-003 are auto-generated and
   pass the existing v0.9-003 fixture round-trip tests.
5. The local artifact registry from v0.14-004 stores at least two
   versions of the reference service and the publishing flow refuses
   downgrade.
6. The three developer modes from v0.14-005 work against the
   reference fleet demo.
7. The Trust Report gains a "Service ecosystem" subsection listing
   published bundles in the local registry with their digests.

## 7. Risk register

| Risk | Mitigation |
|------|------------|
| SDK gaps discovered too late to fix | Author the reference service first; let it block other v0.14 work if needed |
| Lessons-learned doc becomes marketing | Each entry must reference a concrete file:line or workflow step |
| Typed structs generation diverges from catalog | Catalog v1 is frozen; generation is one-time at build, regenerated on catalog change |
| Registry format becomes ad-hoc dependency manager | Explicit non-goal; `--no-deps` semantics enforced |
| Dev modes leak into production | Each mode requires a kernel built with the corresponding feature; production builds refuse |

## 8. Out of scope (beyond §5)

- Live-reload of services. Requires lifecycle changes inconsistent
  with bundle FSM.
- IDE plugins.
- A "service crash dump" facility beyond what audit + GDB already
  provide.
- WASM service hosting (research track).
- Cross-fleet bundle sharing (v1.x).
