# RFC-v0.9-001: Service SDK and Stable Service API Subset

## **Status.** Implemented (v0.9.0)

## Target Version

`v0.9.0`.

## Phase

Developer Service Platform — Epic A (SDK).

## Related Work

- v0.2 RFCs 035-040 — cap / lease / IPC surface.
- v0.5 RFC 004 — semantic catalog v1.
- v0.5 RFC 003 — arch boundary cleanup.
- v0.9 RFCs 002-005 — manifest, semantic toolkit, bundle, QEMU workflow.

---

## 1. Summary

Introduce `fjell-sdk`: a curated Rust crate that exposes a **stable
subset** of Fjell's user-space service surface, with semver guarantees
appropriate to a v0.x.y project (i.e., "no breakage within a v0.x minor
release"). The SDK is the contract an external service developer writes
against — not the entire workspace's crates, which may change between
minor releases without coordination.

The SDK does not introduce new runtime functionality. It is purely a
curation, documentation, and stability layer.

---

## 2. Motivation

By v0.9, the workspace has ~50 crates. External developers writing a
Fjell service today must:

- pick the right combination of cap/lease/IPC types;
- track which crates are stable vs. internal;
- discover the right semantic catalog entry to emit.

`fjell-sdk` collapses all this into a single dependency with a curated
prelude. It also enables v1.0's stability promise by being the *target*
of that promise (rather than the unbounded workspace).

---

## 3. Goals

```text
- `fjell-sdk` crate exporting a curated public surface.
- Re-exports only; no new types in fjell-sdk itself.
- Stable prelude module `fjell_sdk::prelude` covering 90% of needs.
- Doc-tested examples in the SDK crate.
- Stability tier per export: Stable | Provisional | Deprecated.
- A semver-gate CI check that fails any v0.x.(y+1) PR that removes or
  breaks a Stable export.
- A migration shim for at least one deprecation cycle.
```

## 4. Non-Goals

```text
- No semver beyond what cargo provides.
- No multi-language SDK (Rust only in v0.9).
- No FFI / C-bindings.
- No async runtime; SDK matches the synchronous Fjell IPC model.
- No web UI / no GUI.
```

---

## 5. External Design

### 5.1 Crate shape

```text
fjell-sdk/
   src/
      lib.rs                — top-level re-exports + tier markers
      prelude.rs            — convenience prelude
      cap.rs                — capability types (re-exports from fjell-cap)
      ipc.rs                — IPC primitives (re-exports from fjell-ipc)
      lease.rs              — lease primitives
      semantic.rs           — catalog-v1 helpers
      service.rs            — service-skeleton helpers (Service trait)
      net.rs                — Session helpers for enrolled services
      attest.rs             — attestation push helpers
      diagnostics.rs        — bundle helpers
      mod.rs (deprecated)   — none; uses 2018+ module style per project
```

### 5.2 Stability tiers

```rust
/// Stability tier as marked on every public export. Tier is checked by
/// the semver-gate (see §11.2).
#[doc = "stability tier"]
pub enum Tier { Stable, Provisional, Deprecated }
```

Tier is declared via a module-level attribute macro:

```rust
#[fjell_sdk_tier(Stable, since = "v0.9.0")]
pub use fjell_cap::CapHandle;

#[fjell_sdk_tier(Provisional, since = "v0.9.0")]
pub use fjell_ipc::FastPath;

#[fjell_sdk_tier(Deprecated, since = "v0.9.0", removed_in = "v0.10.0")]
pub use fjell_legacy::OldThing;
```

The macro emits structured metadata that the semver-gate consumes.

### 5.3 prelude

```rust
pub mod prelude {
    pub use crate::cap::{CapHandle, CapKind, CapRights};
    pub use crate::ipc::{Endpoint, Message, send_message, recv_message};
    pub use crate::lease::{LeaseId, LeaseExpired};
    pub use crate::service::{Service, ServiceCtx, ServiceMain};
    pub use crate::semantic::{emit_intent, IntentTag};
}
```

The prelude contains only Stable exports.

### 5.4 Service trait

A tiny "service skeleton" that bridges the existing service-manager
ready/health contract:

```rust
pub trait Service: Sized {
    type Config;
    type Error: core::fmt::Debug;
    const NAME: &'static str;

    fn init(ctx: &mut ServiceCtx<'_>, cfg: Self::Config) -> Result<Self, Self::Error>;
    fn ready(&self, ctx: &mut ServiceCtx<'_>) -> Result<(), Self::Error>;
    fn step(&mut self, ctx: &mut ServiceCtx<'_>) -> Result<StepOutcome, Self::Error>;
}

pub enum StepOutcome { Continue, Yield, Done }
```

`ServiceCtx` exposes the v0.2 surface: bootstrap caps, IPC endpoints,
semantic emitter, audit emitter, health-target signaller.

---

## 6. Data Model

### 6.1 Internal metadata table

The `fjell-sdk` crate ships a `.rodata` table emitted by the tier macro:

```text
[
  { path: "CapHandle",  tier: Stable, since: "v0.9.0" },
  { path: "FastPath",   tier: Provisional, since: "v0.9.0" },
  { path: "OldThing",   tier: Deprecated, since: "v0.9.0", removed_in: "v0.10.0" },
  ...
]
```

`fjell-sdk dump-tiers` (a host CLI subcommand) renders this table for
docs.

---

## 7. Internal Design

### 7.1 No new runtime types

The SDK is a re-export crate. The only code it owns is:

- the `Service` trait;
- thin convenience wrappers (e.g., `send_message` that wraps `Endpoint::send`
  with sensible defaults);
- the tier-macro crate `fjell-sdk-macros`.

This keeps the SDK's risk surface small and its maintenance cost low.

### 7.2 Compatibility shims

When a deprecated export is removed at `v0.x.0`, the SDK retains a
*shim module* `fjell_sdk::compat::<old_name>` that exposes the symbol
under its new path for one minor cycle. The shim emits a deprecation
warning at compile time.

### 7.3 Documentation generation

`cargo doc --package fjell-sdk` is the canonical SDK doc. It is published
to the project docs site under `/docs/sdk/`. The tier-macro decorates
each item's rustdoc with a banner showing the tier.

---

## 8. Security Design

This RFC introduces no runtime path. Security considerations:

```text
- The SDK does not export any function that synthesises a Cap; all caps
  flow from cap-broker via ServiceCtx (v0.2 invariant preserved).
- The SDK does not expose attestation signing keys or keyring write
  paths; service developers call typed APIs (e.g.,
  AttestationClient::push) that go through attestd.
- No #[cfg] gated unsafe is re-exported.
```

### 8.1 Audit emission

None at runtime; SDK CI emits build-time diagnostics for tier drift.

---

## 9. Memory / Resource Design

No runtime memory cost.

---

## 10. Compatibility and Migration

- All v0.2-v0.8 services that already depend on `fjell-cap`, `fjell-ipc`,
  etc. directly continue to work. The SDK is *additive*.
- New services introduced under `crates/services/*` (RFC v0.9-002 onwards)
  depend on `fjell-sdk` only.

---

## 11. Test Strategy

### 11.1 Doc-tests

Every Stable export has at least one doc-test:

```text
- CapHandle: minimal "look up a delegated cap" example
- send_message: receive-then-send echo example
- LeaseId: register and observe lease expiry
- Service trait: minimal echo service
- emit_intent: catalog-v1 emit example
- AttestationClient::push: typed push round-trip
```

### 11.2 Semver gate

A new CI step `cargo run -p fjell-sdk-semver-gate`:

```text
- enumerates Stable exports in the current crate
- compares to the manifest checked in at sdk/exports-stable.lock
- failure modes:
    StableExportRemoved
    StableExportSignatureChanged
    StableExportTierLowered
- update path: PR includes `BREAKING-SDK: <export>` and a
  matching ADR entry under docs/src/adr/.
```

### 11.3 Cross-compile check

Build the SDK against the riscv64gc-unknown-none-elf target to confirm
no host-only types leaked into the surface.

---

## 12. Acceptance Criteria

```text
- fjell-sdk + fjell-sdk-macros crates land.
- All re-exports tier-annotated.
- exports-stable.lock checked in.
- ≥ 6 doc-tests pass.
- Semver gate CI step green.
- Cross-compile against riscv64 target green.
- ADR-v0.9-001 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/sdk/overview.md
docs/src/sdk/prelude.md
docs/src/sdk/tier-policy.md
docs/src/sdk/service-trait.md
docs/src/adr/v0.9-001-sdk-as-stability-boundary.md
docs/src/adr/v0.9-001-breaking-sdk-policy.md
```

---

## 14. Open Questions

1. **No-std requirements** — SDK is `no_std + alloc`. Some convenience
   wrappers want `alloc`; that's already in the rest of the user-space
   crates. Acceptable.
2. **Async** — Fjell's IPC is synchronous. A future async layer could be
   added as `fjell-sdk-async` without touching this RFC.
3. **Multi-version coexistence** — can a v0.9 service depend on
   `fjell-sdk = "0.9"` while a v0.10 service uses `"0.10"` in the same
   image? Yes (Rust's crate versioning supports it); SDK avoids global
   statics that would block coexistence.

---

## 15. Release Gate (RFC-local)

```text
- SDK crate ships with full re-export table.
- Doc-tests + semver gate green.
- Tier docs published.
- ADRs Accepted.
```
