# RFC-v0.9-002: Capability Request Manifest and Policy Lint

## Status

Draft (revised, supersedes pack v0.9-002 draft)

## Target Version

`v0.9.0`.

## Phase

Developer Service Platform — Epic B (Manifest / Lint).

## Related Work

- v0.2 cap-broker policy bundle.
- v0.8 RFC 005 — FleetPolicy / CapBrokerPolicy section.
- v0.9 RFC 001 — SDK (the source of the typed manifest API).
- v0.9 RFC 004 — bundle builder (consumes manifests).

---

## 1. Summary

Define **`CapManifest`** — a TOML file shipped *with* each Fjell service
that declares the capabilities, IPC tags, leases, semantic intents, and
audit kinds the service intends to use. Define a host-side **policy
lint** that checks the manifest against:

- the SDK's stable surface (no use of `Deprecated` exports without
  explicit acknowledgement);
- the fleet's CapBrokerPolicy (no requesting rights the policy doesn't
  grant to this service identity);
- the semantic catalog (no emitting intents outside the allow-list for
  the service's role);
- internal cross-checks (every IPC tag claimed-received has a claimed
  endpoint).

The manifest is also bound into the service bundle (RFC v0.9-004) and
the bundle's signed manifest_digest, so manifests cannot be silently
swapped at install.

---

## 2. Motivation

The cap-broker enforces policy at runtime. But:

- developers want to know *at build time* whether a service will be
  granted what it needs;
- operators want to audit "what does this service ask for" without
  reading source code;
- the bundle signer wants a stable artefact to commit to.

The manifest is the artefact; the lint is the build-time check.

---

## 3. Goals

```text
- TOML manifest with stable schema.
- Lint pass on every PR; missing/inconsistent manifest fails CI for a
  service.
- Manifest digest part of the bundle metadata (RFC v0.9-004).
- Lint cross-checks against:
    - SDK tier policy;
    - sample CapBrokerPolicy bundle (per-deployment);
    - semantic catalog v1;
    - manifest internal consistency.
- All claims listed; no implicit rights.
- IDE-friendly: manifest path / line for each lint error.
```

## 4. Non-Goals

```text
- No runtime enforcement based on the manifest (cap-broker is still
  authoritative).
- No automated policy generation. The manifest is a *request*; the
  CapBrokerPolicy is the *grant*.
- No version-pinning of catalog entries beyond catalog-version.
- No JSON / YAML manifests; TOML only.
```

---

## 5. External Design

### 5.1 Manifest location

```text
crates/services/<svcname>/fjell-service.toml
```

The build script (or build-time check) finds the manifest by walking up
from the crate root. CI runs lint over all such files in the workspace.

### 5.2 Manifest example

```toml
[service]
name = "diagnosticsd"
version = "0.9.0"
sdk_min = "0.9.0"

[requires.caps]
endpoint_kinds   = ["Endpoint"]
mmio_kinds       = []                 # no MMIO needed
dma_kinds        = []
interrupt_kinds  = []
session_scopes   = [
    { protocol = "Tcp", peer = "*:4443", purpose = "Diagnostics" },
]
channel_kinds    = ["Diagnostics"]
channel_rights   = ["SXT_RPC_DIAG"]

[requires.ipc]
sends     = ["SXT_DIAG_PUSH"]
receives  = ["SXT_DIAG_ACK"]
endpoints_owned = ["diag-build-endpoint"]

[requires.leases]
lease_kinds = ["Endpoint"]

[requires.semantic]
catalog_version = "1.0"
emits = [
    "DiagnosticBundleBuilt",
    "DiagnosticBundlePushed",
]
subscribes_to = []

[requires.audit]
emits = [
    "DiagnosticBundleBuilt",
    "DiagnosticBundlePushed",
    "DiagnosticPullReceived",
    "DiagnosticPullCompleted",
]

[provides]
ipc_tags_responded = ["DIAG_BUILD_REQ", "DIAG_BUILD_REPLY"]

[deprecations_acknowledged]
sdk = []           # explicit list of Deprecated SDK exports used
```

### 5.3 Lint command

```text
$ fjell-tools manifest lint --svc diagnosticsd
$ fjell-tools manifest lint --workspace
$ fjell-tools manifest lint --workspace --against fleet-policy.bin
```

Output:

```text
[ok]    diagnosticsd
[fail]  recoveryd
  fjell-service.toml:14:1
    error: requested Session scope `proto=Udp, peer=*:53` not granted
           by fleet policy (CapBrokerPolicy v7)
  fjell-service.toml:23:5
    warn: emits "RECOV.UNKNOWN_INTENT" which is not in catalog v1.0
```

---

## 6. Data Model

### 6.1 Manifest schema

```rust
pub const MANIFEST_SCHEMA_VERSION: u16 = 1;

pub struct ServiceManifest {
    pub schema_version: u16,
    pub service:        ServiceHeader,
    pub requires:       Requirements,
    pub provides:       Provisions,
    pub deprecations_acknowledged: DeprecationAck,
    pub manifest_digest: Digest32,
}

pub struct ServiceHeader {
    pub name:     ArrayString<32>,
    pub version:  ArrayString<32>,
    pub sdk_min:  ArrayString<32>,
}

pub struct Requirements {
    pub caps:     CapRequirements,
    pub ipc:      IpcRequirements,
    pub leases:   LeaseRequirements,
    pub semantic: SemanticRequirements,
    pub audit:    AuditRequirements,
}

pub struct CapRequirements {
    pub endpoint_kinds:   ArrayVec<CapKindName, 8>,
    pub mmio_kinds:       ArrayVec<CapKindName, 8>,
    pub dma_kinds:        ArrayVec<CapKindName, 8>,
    pub interrupt_kinds:  ArrayVec<CapKindName, 8>,
    pub session_scopes:   ArrayVec<SessionScopeSpec, 8>,
    pub channel_kinds:    ArrayVec<ChannelKindName, 4>,
    pub channel_rights:   ArrayVec<ChannelRightName, 8>,
}

pub struct SessionScopeSpec {
    pub protocol: L4ProtocolName,
    pub peer:     ArrayString<64>,
    pub purpose:  ArrayString<32>,
}
```

(Other sub-structs mirror the TOML shape; full enumeration in the format
doc.)

### 6.2 Canonical manifest digest

```text
manifest_digest = SHA256(
    "FJELL-MANIFEST-V1" ||
    schema u16 LE ||
    canonical TOML bytes (UTF-8, sorted keys, deterministic whitespace)
)
```

The canonical TOML serializer is part of `fjell-manifest-format`. Hand-
written TOML may not match byte-for-byte; the canonical writer normalises.

---

## 7. Internal Design

### 7.1 Lint passes

```text
Pass 1: SCHEMA
  - parse TOML; reject unknown keys.
  - check required fields present.
  - normalise to canonical form.

Pass 2: INTERNAL
  - every IPC `receives` tag belongs to an `endpoints_owned` endpoint.
  - every `sends` tag has a corresponding upstream service that
    `provides` it (cross-service check across workspace).
  - emitted semantic intents are in catalog version manifest declares.
  - emitted audit kinds are in the workspace audit enum (compile-time
    cross-check).

Pass 3: SDK TIER
  - every imported SDK symbol's tier is Stable, OR
  - the symbol is listed under deprecations_acknowledged.sdk.

Pass 4: POLICY
  - given a CapBrokerPolicy bundle (--against), check every requested
    cap/right against the policy's grants for this service name.

Pass 5: BUNDLE-INTERLOCK
  - manifest_digest matches the digest claimed by the service's bundle
    metadata (RFC v0.9-004 cross-check).
```

### 7.2 Cross-service tag closure

For `sends` tags, the lint builds a graph: service A sends tag T → service
B receives T. Each `sends` edge requires a matching `receives` edge in the
workspace. Missing edges fail lint.

### 7.3 Manifest in build artefacts

The lint emits a sidecar `service.manifest.bin` (canonical binary form
of the parsed manifest with `manifest_digest`). This artefact is
embedded in the service bundle (RFC v0.9-004).

---

## 8. Security Design

### 8.1 What this RFC catches at build time

```text
- Service drift: code requests a cap not in manifest.
- Manifest drift: manifest requests a cap not actually used (warn).
- Policy mismatch: manifest requests a right policy won't grant.
- Catalog drift: emits an intent not in catalog (compile-time fail).
```

### 8.2 What it intentionally does *not* catch

```text
- Runtime requests that vary by config. cap-broker remains authoritative.
- Heuristic detection of "the code probably needs this." Lint is
  declarative, not inferential.
```

### 8.3 Audit emission

None at runtime; lint surfaces failures in CI.

---

## 9. Memory / Resource Design

Build-time tool; no runtime cost. Manifest binary form ≤ 4 KiB per
service.

---

## 10. Compatibility and Migration

- Existing services gain manifests in a coordinated PR; CI is gated on
  manifest presence only after the migration completes.
- Two-phase rollout: warn-only for one minor cycle, then fail.

---

## 11. Test Strategy

### 11.1 Host unit tests (manifest format)

```text
- parse_minimal_manifest
- unknown_key_rejected
- canonical_serialisation_idempotent
- manifest_digest_covers_all_fields
- session_scope_wildcard_accepted
- session_scope_invalid_protocol_rejected
- deprecation_ack_silences_warning
```

### 11.2 Lint integration tests

```text
- workspace_lint_clean_for_all_services
- introducing_undeclared_cap_use_fails_lint     (compile-time test)
- introducing_unknown_emit_intent_fails_lint
- policy_mismatch_against_sample_bundle_fails
```

### 11.3 Negative

| Marker                                                  | Profile  |
|---------------------------------------------------------|----------|
| `NEG:MANIFEST:UNKNOWN_KEY_REJECTED`                     | manifest |
| `NEG:MANIFEST:UNDECLARED_CAP_USE_REJECTED`              | manifest |
| `NEG:MANIFEST:CAT_INTENT_UNKNOWN_REJECTED`              | manifest |
| `NEG:MANIFEST:POLICY_MISMATCH_REJECTED`                 | manifest |
| `NEG:MANIFEST:DIGEST_MISMATCH_WITH_BUNDLE_REJECTED`     | manifest |

(These are CI-only negative tests, not QEMU.)

---

## 12. Acceptance Criteria

```text
- fjell-manifest-format crate.
- fjell-tools manifest lint subcommand.
- All workspace services carry manifests.
- All workspace services' manifests lint clean.
- ≥ 7 host unit + 4 integration tests pass.
- ADR-v0.9-002 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/sdk/manifest.md
docs/src/format/service-manifest.md
docs/src/development/manifest-lint.md
docs/src/adr/v0.9-002-manifest-as-build-artefact.md
docs/src/adr/v0.9-002-canonical-toml.md
```

---

## 14. Open Questions

1. **Per-deployment manifests** — should a service ship variant manifests
   for different deployment profiles (dev / fleet / standalone)? Out of
   scope for v0.9.0; one manifest per service.
2. **Tag namespaces** — currently flat strings. If two services use the
   same tag name for different purposes, lint may flag a false collision.
   Mitigation: namespace tags by service in fjell-ipc itself (future
   RFC).
3. **Generated manifests** — for very large services, hand-maintaining
   the manifest is annoying. A future tool could *propose* a manifest
   diff from compile-time analysis; current scope is hand-written +
   lint.

---

## 15. Release Gate (RFC-local)

```text
- Lint shipped, all manifests in workspace.
- Manifest binary embedded in bundles (v0.9-004 dependency).
- ADRs Accepted.
```
