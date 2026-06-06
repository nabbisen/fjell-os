# RFC-v0.14-002 — First Non-Trivial External Service (Reference)

> **Errata:** This RFC is `Implemented-with-Errata`. Drift recorded as E-006 in `docs/rfcs/ERRATA.md`; reconciled by RFC-v0.16-007.

**Status:** Implemented-with-Errata (v0.14.0)
**Target version:** v0.14.0
**Parent:** v0.14-001.
**Cross-refs:** RFC v0.9-001 (SDK), v0.9-002 (CapManifest),
    v0.9-004 (bundle), v0.9-005 (dev-harness).

## 1. Problem

Every existing Fjell service was authored by the kernel author with
intimate knowledge of internals — `init`, `cap-broker`, `bootctl`,
`netd`, `syncd`, and so on. The SDK was extracted afterwards. v0.14
needs a service authored *by the SDK*, with no privileged access to
internals, to verify the SDK is sufficient.

## 2. Choice of reference service

The candidate must satisfy:

- Exercises capability + lease + IPC together (otherwise we learn
  nothing about the typical author's experience).
- Emits semantic intents (exercises the catalog path).
- Reads at least one persistent state (exercises storaged).
- Optionally exercises networking (v0.4) — desirable but not required.
- Has plausible standalone value (someone might actually want it).
- Small enough to land in v0.14 without dominating the milestone.

### 2.1 Selected service: `fjell-config-sync`

A configuration sync agent for archetype A2 (sensor/edge fleet nodes).
Watches a local configuration store, applies remote updates (signed),
emits a measurement record of the active configuration digest.

Justification:

- Capabilities used: `PersistentStore` (read+write), `Endpoint`
  (receive updates), `AuditDrain` (emit intents). Three is enough to
  exercise composition.
- Leases: one per attached store handle.
- IPC: receives `CONFIG_UPDATE` messages from `secure-transportd`
  (v0.4-003), replies with `CONFIG_APPLIED` or `CONFIG_REJECTED`.
- Semantic intents: emits `CONFIG.UPDATED`, `CONFIG.REJECTED`,
  `CONFIG.DIGEST_REPORTED`.
- Persistent state: a small KV store of current config.
- No new kernel surface required.

### 2.2 Alternative candidates considered

- **Metrics exporter.** Too IPC-light.
- **Sensor poller.** Hardware-dependent; v0.12 board would block.
- **Watchdog tickle service.** Too trivial.
- **Remote logger.** Overlaps with audit; would reinvent v0.4-005.

`fjell-config-sync` is the most pedagogically useful: it touches
every part of the SDK an A1/A2 author would touch.

## 3. Layout

```text
crates/fjell-config-sync/
  Cargo.toml                — depends only on fjell-sdk
  cap-manifest.toml         — RFC v0.9-002 manifest
  src/
    main.rs                 — entry point + main loop
    store.rs                — persistent state interface
    handler.rs              — IPC message handlers
    digest.rs               — config digest computation
  README.md                 — what it does, how to deploy
  tests/
    unit.rs                 — host unit tests
```

The Cargo.toml has **exactly one** Fjell dependency:

```toml
fjell-sdk = { path = "../fjell-sdk" }
```

Reaching past the SDK boundary is a build-time error caught by a new
CI lint `ci-sdk-purity` (added in this RFC).

## 4. Authoring discipline

The author of this service must:

- Treat the SDK as the only Fjell-side surface.
- Read the SDK docs alone before consulting any other Fjell source.
- Log every moment of "I need X and the SDK doesn't expose it" to the
  lessons-learned doc.
- Submit one PR per logical step (manifest, scaffolding, handlers,
  store, tests, bundle) so the experience trace is reviewable.

## 5. Capability manifest

The committed `cap-manifest.toml`:

```toml
service     = "fjell-config-sync"
sdk_api_rev = 1
caps        = ["Endpoint", "PersistentStore", "AuditDrain"]
rights      = ["SEND", "RECV", "READ", "WRITE", "AUDIT_DRAIN"]
ipc_tags    = [
  "v0_7::CONFIG_UPDATE",
  "v0_7::CONFIG_QUERY",
  "tags::READY"
]
intents     = [
  0x0501,  # CONFIG.UPDATED
  0x0502,  # CONFIG.REJECTED
  0x0503,  # CONFIG.DIGEST_REPORTED
]
```

The catalog tags 0x0501..0x0503 are allocated as part of this RFC in
a reserved range owned by `fjell-config-sync` (matches the catalog
ownership model from RFC v0.7.5-001). Schema for each is committed
alongside.

## 6. Bundle

The service is built into a bundle via `cargo xtask bundle build` and
signed with the test-mode key. The bundle is committed to the local
registry (v0.14-004) at version `0.1.0`.

A second version `0.1.1` is also published to exercise upgrade /
downgrade-refusal semantics.

## 7. Deployment in the reference fleet

The v0.10-005 three-node fleet demo gains an optional step:

```
cargo xtask fleet-demo deploy --service fjell-config-sync --version 0.1.0
```

After deployment, the operator can:

- Send a config update via `secure-transportd`.
- Observe the digest change in audit intents.
- Trigger a downgrade and observe refusal.
- Run a re-attestation (v0.13-004) and see `CONFIG.DIGEST_REPORTED`
  in the resulting manifest.

## 8. Lessons-learned doc

`docs/sdk/lessons-from-v0.14.md` is appended throughout v0.14-002's
authoring. Each entry follows:

```text
## L<n>: <short title>
Date: YYYY-MM-DD
Phase: scaffolding | manifest | handlers | bundle | deployment | tests
File: <link to point in service code>
What happened: ...
Why it was hard: ...
Resolution: applied a workaround / filed RFC-v0.14.x | filed v1.x backlog
```

An empty lessons file at v0.14 landing would be a red flag.

## 9. Acceptance criteria

1. `crates/fjell-config-sync/` exists with the §3 layout.
2. The Cargo.toml's Fjell dependency surface is `fjell-sdk` only;
   `ci-sdk-purity` enforces this.
3. The CapManifest passes `cargo xtask dev lint`.
4. Bundle built, signed, and published to the local registry.
5. Service deploys to the reference fleet and processes at least one
   real config update end-to-end.
6. `docs/sdk/lessons-from-v0.14.md` has ≥ 1 actionable entry.
7. Three new catalog intents allocated, owners assigned, schemas
   committed.
8. Unit tests pass; integration test (deploy + update + assert audit
   record) passes via QEMU.

## 10. Out of scope

- Production-grade `fjell-config-sync`. The reference is intentionally
  minimal.
- Multiple reference services. One is enough to validate the SDK; a
  second would dilute focus.
- Authoring a service in a non-Rust language.
- Service hot-update (lifecycle changes excluded; v1.x territory).
