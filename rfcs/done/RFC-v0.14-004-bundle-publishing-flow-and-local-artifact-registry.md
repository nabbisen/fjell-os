# RFC-v0.14-004 — Bundle Publishing Flow and Local Artifact Registry

**Status:** Implemented (v0.14.0)
**Target version:** v0.14.0
**Parent:** v0.14-001.
**Cross-refs:** RFC v0.9-004 (bundle format), v0.11-003 (signing).

## 1. Problem

RFC v0.9-004 defines the bundle wire format. RFC v0.11-003 ships the
signing pipeline. There is still no defined *flow* for how a bundle
moves from "freshly built on the developer host" to "available for a
fleet coordinator to deploy." Without this, the bundle format is an
artefact in search of a workflow.

v0.14 lands a simple, file-based **local artifact registry** plus a
publish/install flow that exercises every authentication boundary.

The local registry is deliberately not a "package manager." It is a
file system layout with a manifest, suitable for an operator running
on their own infrastructure or for the reference fleet demo. Anything
more is post-v1.0.

## 2. Registry layout

```text
registry/
  registry.toml                     — top-level manifest
  bundles/
    <service-name>/
      <version>/
        bundle.bundle               — ServiceBundle wire bytes
        bundle.bundle.sig           — SignedManifest (v0.11-003)
        cap-manifest.toml           — committed manifest snapshot
        meta.toml                   — per-version metadata
  index/
    by-digest/                      — symlink digest → bundle path
    by-service/                     — symlink service-name → latest
```

### 2.1 `registry.toml`

```toml
schema_version = 1
created_at_ns  = 1727000000000000000
signing_keys   = [
  { key_id = "00112233...", role = "publisher", state = "active" },
]
allow_unsigned = false
```

### 2.2 `meta.toml` per version

```toml
service_name      = "fjell-config-sync"
version           = "0.1.0"
sdk_api_rev       = 1
catalog_version   = 1
manifest_digest   = "ab cd ..."
bundle_digest     = "..."
sig_key_id        = "..."
published_at_ns   = ...
prev_version      = ""              # empty for first
notes             = "initial reference build"
```

## 3. Publish flow

```
cargo xtask publish \
    --bundle    target/.../fjell-config-sync.bundle \
    --sig       target/.../fjell-config-sync.bundle.sig \
    --registry  ./registry \
    --version   0.1.0 \
    --notes     "..."
```

Steps:

1. Verify the bundle (`fjell-bundle-format::verify_bundle`).
2. Verify the signature against `registry.toml`'s `signing_keys`.
3. Refuse if `service_name + version` already exists.
4. Refuse if the new version is not strictly greater than the latest
   committed version for the same service. **No downgrade publishing.**
5. Copy artefacts into `registry/bundles/<svc>/<ver>/`.
6. Update `meta.toml`, `index/by-digest`, `index/by-service`.
7. Emit a `REGISTRY.PUBLISHED` audit record into the publishing
   host's audit chain.

## 4. Install flow

```
cargo xtask install \
    --service   fjell-config-sync \
    --version   0.1.0 \
    --registry  ./registry \
    --target    <fleet-node-id-or-local>
```

Steps:

1. Resolve `service + version` to a registry path.
2. Re-verify bundle + signature.
3. Verify signing key against the *destination's* trust anchors —
   the publishing host's trust does not transitively apply.
4. Refuse install if destination already has a higher version of the
   same service (downgrade-refusal mirrors v0.9-004 §5.3).
5. Hand the bundle to the destination's installer pipeline; lifecycle
   proceeds per v0.9-004 (Fetched → Verified → Committed → Running →
   Confirmed | RolledBack).

## 5. Versioning rules

The registry enforces:

- SemVer-like ordering: `1.2.3 < 1.2.4 < 1.3.0 < 2.0.0`.
- The set of monotonic counters is total within a service name.
- Major-version bumps cannot land if the new major's `sdk_api_rev`
  exceeds the registry's host `SDK_API_REV`.
- A bundle whose `catalog_version` is less than the registry's
  declared minimum is refused.

These rules are mechanically enforced; the operator cannot bypass via
flag without an explicit `--force-policy-override` that itself emits
a loud audit record.

## 6. What the registry is *not*

Sharp boundaries to prevent scope drift:

- No dependency resolution. A bundle is self-contained.
- No transitive trust. Signing a bundle does not sign other bundles.
- No remote operation. The "registry" is a directory on disk; the
  publisher and the installer run against the same filesystem (or a
  filesystem mounted equivalently). Network-served registries are a
  v1.x conversation.
- No history-rewriting. Published versions are immutable; corrections
  ship as new versions.

## 7. Reference registry

The reference fleet demo (RFC-v0.10-005) gains a `registry/` directory
at landing, populated by v0.14-002's two reference-service versions
(0.1.0 and 0.1.1). The demo's `fleet-demo deploy` step now reads from
this registry instead of taking a raw bundle path.

## 8. CI coverage

A new `test-all` host tier step:

- Publish 0.1.0 → succeeds.
- Publish 0.0.9 → refused (downgrade).
- Publish 0.1.0 again → refused (duplicate).
- Install 0.1.0 against the QEMU reference fleet → succeeds.
- Tamper bundle bytes → install refused with `Tampered`.
- Tamper signature bytes → install refused with `SigVerifyFailed`.

## 9. Acceptance criteria

1. `cargo xtask publish` and `cargo xtask install` exist.
2. `registry/registry.toml` and per-version `meta.toml` formats are
   defined and parseable.
3. The downgrade-refusal rules from §3 and §4 are enforced.
4. `--force-policy-override` exists and produces a `REGISTRY.OVERRIDE`
   audit record.
5. Reference fleet demo deploys from the registry rather than raw
   bundle path.
6. CI exercises the §8 cases.
7. Trust Report includes registry contents (count of bundles per
   service, latest version, key fingerprints).

## 10. Out of scope

- Network-served registries.
- Mirror / proxy semantics across registries.
- Dependency resolution.
- Web UI for browsing the registry.
- Multi-organisation registry sharing.
- Pre-release / nightly channels. The registry holds released
  versions only.
