# RFC-v0.9-004: Bundle Builder and Signed Service Package

## **Status.** Implemented (v0.9.0)

## Target Version

`v0.9.0`.

## Phase

Developer Service Platform — Epic D (Bundle / Package).

## Related Work

- v0.3 RFC 003 — ReleaseMetadata (the OS-level analogue).
- v0.4 RFC 004 — staged upgrade pipeline.
- v0.9 RFCs 001/002/003 — SDK, manifest, semantic.
- v0.8 RFC 003 — RolloutPlan (service bundles can be staged like OS releases).

---

## 1. Summary

Define **`ServiceBundle`** — a signed, self-describing artefact that
packages a Fjell service binary with its CapManifest (RFC v0.9-002),
semantic catalog dependency, SDK version, and optional configuration
schema. Define the **bundle builder** (`fjell-tools bundle build`) that
assembles a bundle from a Cargo crate + manifest, computes the canonical
bundle digest, and signs it.

Service bundles are how user-supplied services arrive at a Fjell node.
The kernel never loads a bundle directly; `service-manager` installs the
binary into a staged slot and registers the bundle's metadata with
storaged.

---

## 2. Motivation

By v0.9 the system supports user-developed services. Without a bundle
format, install is "copy the binary somewhere and hope" — which provides
no integrity, no auditability, and no policy hook. A bundle:

- gives one signed artefact to integrity-check;
- carries the manifest that cap-broker consults;
- carries dependency assertions (SDK version, catalog version) that
  service-manager validates before launch;
- is stage-able through the same v0.4 RFC 004 pipeline as OS releases.

---

## 3. Goals

```text
- Self-describing bundle: binary + manifest + dependency assertions.
- Signed via KeyPurpose::PolicyVerification (or a dedicated
  ServiceSigning purpose; see §14.1).
- Bundle digest canonical and stable.
- Builder (`fjell-tools bundle build`) reproducible: same inputs
  → byte-identical bundle.
- Installer pipeline mirroring OS upgrade (Fetched → Verified →
  Committed → Confirmed).
- Per-service rollback on health failure.
- Multiple bundles installable per system, each with its own slot.
```

## 4. Non-Goals

```text
- No package manager: no dependency resolution between bundles.
- No multi-version coexistence per service (one active version).
- No bundle subsystem download from third-party indices.
- No bundle for OS-level components (those use RFC v0.4-004 / v0.3-003
  paths).
- No dynamic linking / plugin model.
```

---

## 5. External Design

### 5.1 Developer workflow

```text
$ cargo build -p my-service --target riscv64gc-unknown-none-elf --release
$ fjell-tools manifest lint --svc my-service
$ fjell-tools bundle build --svc my-service \
       --binary target/.../my-service.elf \
       --manifest crates/services/my-service/fjell-service.toml \
       --out my-service-0.1.0.bundle
$ fjell-tools bundle sign --key svc-signing.key --in my-service-0.1.0.bundle
$ fjell-tools bundle inspect my-service-0.1.0.bundle
```

### 5.2 Operator workflow

```text
$ fjell-tools service stage my-service-0.1.0.bundle
$ fjell-tools service list
$ fjell-tools service confirm my-service-0.1.0
$ fjell-tools service abort my-service-0.1.0
```

### 5.3 Bundle file layout

```text
ServiceBundle = {
    header        (canonical encoding, fixed shape)
    manifest_blob (canonical TOML bytes from RFC v0.9-002)
    config_schema (optional, JSON-subset)
    binary_blob   (ELF bytes, exact)
    signature     (64 B Ed25519)
}
```

Concrete on-disk format is a single file using a TLV pattern:

```text
[u32 magic 'FJSB'] [u16 schema_version] [u16 section_count]
for each section: [u16 tag] [u32 length] [u8; length payload]
[Digest32 bundle_digest] [Signature signature]
```

---

## 6. Data Model

### 6.1 Header & metadata

```rust
pub const SERVICE_BUNDLE_VERSION: u16 = 1;

pub struct BundleHeader {
    pub schema_version:     u16,
    pub service_name:       [u8; 32],
    pub service_version:    [u8; 16],
    pub sdk_min:            [u8; 16],
    pub catalog_version:    [u8; 8],          // e.g. "1.0\0\0\0\0\0"
    pub target_triple:      [u8; 32],         // "riscv64gc-unknown-none-elf"
    pub manifest_digest:    Digest32,
    pub binary_digest:      Digest32,
    pub binary_size:        u64,
    pub config_schema_digest: Digest32,      // zeroed if absent
    pub created_tick:       u64,
    pub bundle_digest:      Digest32,
}

pub struct SignedServiceBundle {
    pub header:         BundleHeader,
    pub manifest_blob:  ArrayVec<u8, 4096>,
    pub config_schema:  ArrayVec<u8, 2048>,
    pub binary_blob:    Vec<u8>,             // streamed at install time
    pub signature:      Signature,
}
```

### 6.2 Canonical bundle digest

```text
bundle_digest = SHA256(
    "FJELL-SVC-BUNDLE-V1" ||
    schema u16 LE || service_name 32 B || service_version 16 B ||
    sdk_min 16 B || catalog_version 8 B || target_triple 32 B ||
    manifest_digest 32 B || binary_digest 32 B || binary_size u64 LE ||
    config_schema_digest 32 B || created_tick u64 LE
)
```

Signing domain: `"FJELL-SVC-BUNDLE-SIGN-V1"`.

### 6.3 Service slot

```rust
pub struct ServiceSlot {
    pub schema_version:    u16,
    pub service_name:      [u8; 32],
    pub installed_version: [u8; 16],
    pub bundle_digest:     Digest32,
    pub binary_digest:     Digest32,
    pub manifest_digest:   Digest32,
    pub state:             ServiceSlotState,
    pub installed_tick:    u64,
    pub confirmed_tick:    u64,
    pub slot_digest:       Digest32,
}

#[repr(u8)]
pub enum ServiceSlotState {
    Empty          = 0,
    Fetched        = 1,
    Verified       = 2,
    Committed      = 3,
    AwaitingHealth = 4,
    Confirmed      = 5,
    Failed         = 6,
    Aborted        = 7,
}
```

Persisted via storaged kind `ServiceSlot = 0x1B`.

---

## 7. Internal Design

### 7.1 Install pipeline (`service-manager` extension)

```text
on stage(bundle_path):
  parse bundle; verify magic/schema
  verify bundle_digest matches recomputation
  verify signature against pinned ServiceSigning anchor
  parse manifest_blob via fjell-manifest-format
  cross-check manifest_digest matches header
  lint manifest against current CapBrokerPolicy (RFC v0.9-002 lint
    embedded as a library)
  reject if requirements exceed grants
  verify catalog_version is currently active (catalog v1)
  verify sdk_min is satisfied by running SDK version
  verify target_triple matches running platform
  write binary to staged slot (per-service A/B not required in v0.9.0;
    single slot per service)
  storaged.append ServiceSlot { state = Verified, ... }
  audit: ServiceBundleVerified { service_name, version }

on commit(service_name):
  if slot.state != Verified: Err(BadTransition)
  service-manager.activate(binary_path, manifest)
  storaged.append ServiceSlot { state = Committed }

on confirm(service_name):
  if service has not entered AwaitingHealth: Err(BadTransition)
  storaged.append ServiceSlot { state = Confirmed }
  audit: ServiceConfirmed { service_name }

on health_fail(service_name):
  service-manager.deactivate(binary_path)
  storaged.append ServiceSlot { state = Failed }
  audit: ServiceFailed { service_name, reason_code }
```

### 7.2 Reproducible builds

The bundle builder:

- normalises file timestamps in the ELF (uses 0 epoch in target metadata
  where possible);
- sorts manifest TOML canonical form (RFC v0.9-002);
- writes sections in declared order;
- uses deterministic created_tick (operator-supplied via flag, default 0).

A `--strict-reproducible` flag asserts the output matches a previous
build given the same inputs.

### 7.3 Configuration schema

`config_schema` (when present) is a JSON-subset that the service uses to
declare permitted configuration keys, types, and bounds. configd
validates incoming configs against this schema.

---

## 8. Security Design

### 8.1 Threat model deltas

```text
Threat T-250: Adversary supplies a bundle with a valid signature but a
              manifest requesting more than the policy allows.
Mitigation:  install-time policy lint runs against the running
             CapBrokerPolicy; mismatched manifest rejected even with
             valid signature.

Threat T-251: Bundle binary swapped between sign and install.
Mitigation:  binary_digest covered by bundle_digest covered by signature.

Threat T-252: Cross-platform bundle install (e.g., a riscv64 bundle on
              an arm64 node).
Mitigation:  target_triple covered by digest; service-manager rejects
             mismatched triple at install time.

Threat T-253: Replay of an older bundle version to "downgrade" a
              service.
Mitigation:  service_version comparison; if installed_version > new
              version, install requires --allow-downgrade flag and emits
              an audit event ServiceVersionDowngrade.

Threat T-254: Service-signing key compromise affects all services.
Mitigation:  KeyPurpose::ServiceSigning (or a per-vendor anchor) is
             rotatable via RFC v0.3-002 keyring rotation; old anchor
             retired by epoch advance.

Threat T-255: Catalog version drift: bundle built against catalog
              v1.0 installs on a node where v1.0 has been retired.
Mitigation:  catalog_version covered by digest; install-time check
             rejects if the catalog version is no longer active.
```

### 8.2 Audit emission

```text
ServiceBundleParsed         { service_name, version, bundle_digest }
ServiceBundleVerified       { service_name, version }
ServiceBundleRejected       { service_name, version, error_code }
ServiceInstalled            { service_name, version, slot_digest }
ServiceFailed               { service_name, version, reason_code }
ServiceVersionDowngrade     { service_name, old, new }
```

---

## 9. Memory / Resource Design

- Header + manifest + config_schema ≤ ~7 KiB.
- Binary blob sized to fit in a per-service slot (1 MiB default).
- service-manager registry: bounded to MAX_SERVICES = 32 active slots.

---

## 10. Compatibility and Migration

- Existing OS-bundled services are *built in* to the OS image and not
  shipped as bundles.
- A migration path can convert an in-tree service to a bundle by
  building a CapManifest, then a bundle, and removing it from the OS
  image — but this is optional for v0.9.

---

## 11. Test Strategy

### 11.1 Host unit tests

```text
- bundle_digest_covers_binary
- bundle_digest_covers_manifest_digest
- bundle_digest_covers_target_triple
- bundle_serialise_then_parse_round_trip
- bundle_signature_round_trip
- target_mismatch_rejected
- sdk_min_unsatisfied_rejected
- catalog_version_inactive_rejected
- manifest_policy_lint_failure_rejects_install
- reproducible_build_two_runs_match
- service_slot_state_machine_transitions
```

### 11.2 QEMU smoke

```text
- SMOKE:BUNDLE:BUILD_AND_INSTALL_HAPPY
- SMOKE:BUNDLE:ROLLBACK_ON_HEALTH_FAIL
- SMOKE:BUNDLE:UPGRADE_IDEMPOTENT
```

### 11.3 Negative

| Marker                                                  | Profile  |
|---------------------------------------------------------|----------|
| `NEG:BUNDLE:BAD_SIGNATURE_REJECTED`                     | bundle   |
| `NEG:BUNDLE:DIGEST_MISMATCH_REJECTED`                   | bundle   |
| `NEG:BUNDLE:TARGET_TRIPLE_MISMATCH_REJECTED`            | bundle   |
| `NEG:BUNDLE:CATALOG_INACTIVE_REJECTED`                  | bundle   |
| `NEG:BUNDLE:SDK_MIN_REJECTED`                           | bundle   |
| `NEG:BUNDLE:POLICY_LINT_AT_INSTALL_REJECTED`            | bundle   |
| `NEG:BUNDLE:DOWNGRADE_REQUIRES_FLAG`                    | bundle   |

---

## 12. Acceptance Criteria

```text
- fjell-bundle-format + builder + installer integration land.
- service-manager extended for slot states.
- 11 host tests + 3 SMOKE + 7 NEG markers green.
- Reproducible-build flag works.
- ADR-v0.9-004 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/architecture/v0.9-004-service-bundle.md
docs/src/format/service-bundle.md
docs/src/sdk/build-and-install.md
docs/src/adr/v0.9-004-service-signing-purpose.md
docs/src/adr/v0.9-004-reproducible-bundles.md
```

---

## 14. Open Questions

1. **Service signing purpose** — `PolicyVerification` reuse vs. a
   dedicated `ServiceSigning` purpose. Recommendation: introduce
   `KeyPurpose::ServiceSigning = 0x09` in this RFC; explicit purpose
   makes rotation policy independent.
2. **Per-service A/B** — single slot per service in v0.9.0. A/B for
   services is a v0.9.x feature when service upgrades need fail-safe.
3. **Service-bundle rollout governance** — RolloutPlan currently scopes
   to OS counters. A future v0.9.x RFC extends to per-service plans.

---

## 15. Release Gate (RFC-local)

```text
- Bundle format + builder + installer land.
- 11 host + 3 SMOKE + 7 NEG markers green.
- New KeyPurpose::ServiceSigning wired into keyring.
- ADRs Accepted.
```
