# RFC-v0.3-001: HardwareTrustProvider Interface and Provider Registry

**Status.** Implemented (v0.3.0)

## Status

Draft (revised, supersedes pack v0.3-001 draft)

## Target Version

`v0.3.0` (foundation; the trait and registry must land in `v0.3.0` proper).

## Phase

Hardware Trust Abstraction and Local Update Hardening — Epic A (Trust Provider Boundary).

## Related Work

- v0.2 RFC 041 — *Persistent Evidence Hardening* (defines the audit/measurement
  evidence that this RFC's providers must produce signatures over).
- v0.2 RFC 040 — *cap-broker Bootstrap and Default Deny* (the registry handoff
  pattern is borrowed from cap-broker).
- v0.3 RFCs 002, 003, 004 — depend on this RFC.

---

## 1. Summary

Introduce a provider-neutral `HardwareTrustProvider` trait, a fixed-capacity
`ProviderRegistry` with a one-way bootstrap-to-enforcing handoff, and three
concrete provider implementations (`DevelopmentTrustProvider`,
`NullTrustProvider` for negative testing, and a placeholder `TpmTrustProvider`
deferred to v0.3.x). User-space services — `verifyd`, `attestd`, `measuredd`,
`upgraded` — replace their hard-coded development-grade signature paths with
calls through the trait.

The kernel is **not** modified by this RFC. The trust provider boundary lives
entirely in user space; the kernel continues to act as a mechanism layer that
checks capabilities and ferries IPC messages.

---

## 2. Motivation

v0.2 closed the local security boundary but left two open seams:

1. **Vendor coupling risk.** The development-grade Ed25519 verification in
   `verifyd` is implemented in-line. If the project later adds TPM 2.0 or DICE
   support, the temptation will be to fork `verifyd` per platform. That would
   re-couple policy code to hardware.
2. **No abstraction over rollback counters.** v0.2 has no mechanism for a
   monotonic counter. Anti-rollback (RFC v0.3-003) needs one, but where it
   lives — kernel, `storaged`, hardware fuse — must be a provider decision,
   not a `upgraded` decision.

The trust provider trait introduces the abstraction *now*, before either of
these become entangled. The trait is narrow (five methods), the data is
explicit (fixed-size, no `alloc`), and the registry is observable from the
semantic stream.

---

## 3. Goals

```text
- Single Rust trait that abstracts: measurement read, attestation sign,
  rollback counter read, key seal, key unseal.
- One development provider that runs deterministically in QEMU.
- One null provider for negative tests (must be rejected in release profile).
- A fixed-capacity registry that supports bootstrap → enforcing handoff.
- All provider state observable in the semantic stream and audit log.
- No kernel modification.
- No alloc; the crate is `no_std` and usable from existing user-space services.
```

## 4. Non-Goals

```text
- No TPM 2.0 implementation in v0.3.0 (placeholder kind only; impl is v0.3.x).
- No DICE implementation in v0.3.0 (placeholder kind only).
- No remote attestation transport (v0.4 RFC 005).
- No new syscall, no new cap kind. Providers run in user space behind the
  existing IPC capability surface.
- No mandatory rotation policy. Rotation is a provider-internal concern
  exposed only via the `epoch` field on `SealedKey`.
- No general-purpose KMS. Sealed keys are addressable by `KeyPurpose`, not by
  arbitrary string.
```

---

## 5. External Design

### 5.1 User-visible behavior

From the perspective of `verifyd` and `attestd`, the change is a refactor:
where today they call `sign_local_dev(...)` and `verify_local_dev(...)`, after
this RFC they call `provider.sign_attestation(...)` and
`provider.read_measurement(...)`. The semantic stream gains a new event kind,
`TrustProviderStateChanged`, emitted whenever a provider transitions between
`Bootstrap → Active → Faulted → Withdrawn`.

From the perspective of an operator inspecting the system via the text proxy,
a new section appears in the system summary:

```text
Trust Providers:
  id=1  kind=Development  profile=FjellLocalV1  state=Active   gen=1  name=fjell-dv
```

### 5.2 Provider lifecycle

```text
   register()      enter_enforcing()
Empty ────────► Bootstrap ───────────► Enforcing
                  │                        │
                  │ replace()              │ replace() (release-profile only)
                  ▼                        ▼
                generation++             generation++

  fault detected         remove()
Active ───────────► Faulted ─────► Withdrawn
```

The transitions to `Faulted` are observable but recoverable: a `replace()`
call from the owning service re-initialises the slot with a higher generation
and the old `ProviderHandle` becomes stale.

---

## 6. Data Model

### 6.1 Identifiers

```rust
pub struct TrustProviderId(pub u32);   // 0 = UNSET (sentinel)

pub struct ProviderHandle {
    pub id: TrustProviderId,
    pub generation: u16,   // increments on every slot replace/remove
}
```

### 6.2 Provider kind, profile, capabilities, state

```rust
#[repr(u8)]
pub enum TrustProviderKind { Development = 1, Tpm = 2, Dice = 3, Null = 4 }

#[repr(u8)]
pub enum TrustProfile { FjellLocalV1 = 1, FjellTpmV1 = 2, FjellDiceV1 = 3 }

pub struct TrustProviderCapabilities(pub u32);
//   bit 0: READ_MEASUREMENT
//   bit 1: SIGN_ATTESTATION
//   bit 2: READ_ROLLBACK_COUNTER
//   bit 3: SEAL_KEY
//   bit 4: UNSEAL_KEY
// bits 5-31: reserved (MBZ)

#[repr(u8)]
pub enum TrustProviderState { Bootstrap = 1, Active = 2, Faulted = 3, Withdrawn = 4 }
```

### 6.3 Public descriptor

```rust
pub struct TrustProviderDescriptor {
    pub id:           TrustProviderId,
    pub kind:         TrustProviderKind,
    pub profile:      TrustProfile,
    pub capabilities: TrustProviderCapabilities,
    pub state:        TrustProviderState,
    pub generation:   u16,
    pub name:         [u8; 8],   // ASCII, zero-padded
}
```

The descriptor is the only public projection of a provider. Keys, signatures,
measurement bytes, and rollback counter values flow exclusively through the
trait methods.

### 6.4 Key material types

```rust
pub const SIGNATURE_LEN: usize = 64;   // accepts Ed25519, ECDSA-P256

pub struct Signature { pub bytes: [u8; SIGNATURE_LEN], pub len: u8 }

pub struct AttestationDigest(pub Digest32);

pub struct KeyMaterial { pub bytes: [u8; 64], pub len: u8 }
// Debug impl REDACTS bytes; PartialEq compares only the meaningful slice.

pub struct SealedKey {
    pub purpose:  KeyPurpose,
    pub blob:     [u8; 96],
    pub blob_len: u8,
    pub epoch:    u32,   // provider-internal; advisory only
}
```

### 6.5 Key purposes

```rust
#[repr(u8)]
pub enum KeyPurpose {
    ReleaseVerification = 0x01,
    RootfsVerification  = 0x02,
    PolicyVerification  = 0x03,
    AttestationSigning  = 0x04,
    SealedDataKey       = 0x05,
    SnapshotSigning     = 0x06,   // reserved for v0.7
}
```

Adding a purpose is a security-boundary change requiring an ADR.

### 6.6 Errors

`TrustError` is `#[repr(u16)]`; codes are stable and projected into audit
records. See `crates/fjell-trust-provider/src/error.rs` for the full table.

```text
0x0001 NotSupported
0x0002 ProviderUnavailable
0x0003 NullProviderForbidden
0x0004 StaleHandle
0x0005 PurposeMismatch
0x0006 SealIntegrityFailed
0x0007 RollbackCounterExhausted
0x0008 KeyMaterialTooLarge
0x0009 SignFailed
0xFFFF Internal
```

---

## 7. Internal Design

### 7.1 Trait

```rust
pub trait HardwareTrustProvider {
    fn provider_id(&self) -> TrustProviderId;
    fn descriptor(&self) -> TrustProviderDescriptor;

    fn read_measurement(&self) -> Result<MeasurementHead, TrustError>;
    fn sign_attestation(&self, input: AttestationDigest) -> Result<Signature, TrustError>;
    fn read_anti_rollback_counter(&self) -> Result<u64, TrustError>;
    fn seal_key(&self, purpose: KeyPurpose, key: KeyMaterial) -> Result<SealedKey, TrustError>;
    fn unseal_key(&self, purpose: KeyPurpose, sealed: &SealedKey) -> Result<KeyMaterial, TrustError>;
}
```

Every method except `provider_id` and `descriptor` has a default
implementation returning `TrustError::NotSupported`, so a partial provider
need only implement the capabilities it advertises.

### 7.2 Registry (`ProviderRegistry`)

```rust
pub const MAX_PROVIDERS: usize = 8;

pub enum RegistryPhase { Bootstrap, Enforcing }

pub enum RegistryError {
    CapacityExhausted,
    PhaseLocked,
    NullProviderForbidden,
    StaleHandle,
    NotFound,
}

pub struct ProviderRegistry {
    slots:    [Slot; MAX_PROVIDERS],
    phase:    RegistryPhase,
    next_id:  u32,
}

impl ProviderRegistry {
    pub const fn new() -> Self;
    pub fn phase(&self) -> RegistryPhase;
    pub fn enter_enforcing(&mut self);
    pub fn register(&mut self, descriptor: TrustProviderDescriptor)
        -> Result<ProviderHandle, RegistryError>;
    pub fn lookup(&self, h: ProviderHandle)
        -> Result<TrustProviderDescriptor, RegistryError>;
    pub fn replace(&mut self, h: ProviderHandle, new: TrustProviderDescriptor)
        -> Result<ProviderHandle, RegistryError>;
    pub fn remove(&mut self, h: ProviderHandle) -> Result<(), RegistryError>;
    pub fn descriptors(&self) -> impl Iterator<Item = TrustProviderDescriptor> + '_;
    pub fn len(&self) -> usize;
}
```

### 7.3 Bootstrap handoff

The owning service (initially `verifyd`) drives the registry in three steps:

```text
1. registry = ProviderRegistry::new();              // Bootstrap
2. registry.register(dev_descriptor)?;              // ok in Bootstrap
   registry.register(null_descriptor)? -> ok        // ok in Bootstrap
                                                     //   (negative-test fixture)
3. registry.enter_enforcing();                      // one-way transition
   registry.register(null_descriptor)
     -> Err(NullProviderForbidden)                  // proves negative test
```

Once `enter_enforcing()` is called, the only mutating API that succeeds is
`replace()`, and even then a `Null` descriptor is rejected.

### 7.4 Provider implementations

- `DevelopmentTrustProvider` — software-only, deterministic, used in
  `verifyd`/`attestd` for QEMU and host tests. Signatures are
  `SHA256(TRUST_DOMAIN || provider_id || dev_key || digest)[0..32]`. Sealing
  is `MAC-then-XOR` with the dev key. All operations succeed when the
  descriptor state is `Bootstrap` or `Active`.
- `NullTrustProvider` — always returns `NotSupported`. Used as a fixture to
  prove `NullProviderForbidden` rejection.
- `TpmTrustProvider` — stub only in v0.3.0 (an empty struct + `Kind::Tpm`
  descriptor). Full implementation in a later v0.3.x RFC.

---

## 8. Security Design

### 8.1 Threat model deltas vs v0.2

```text
Threat T-30: A malicious service obtains a signing capability and tries to
             forge an attestation record.
Mitigation:  attestd holds the only cap to the AttestationSigning provider
             method; cap-broker enforces no other service may obtain it.

Threat T-31: A test-only Null provider is shipped to production.
Mitigation:  enter_enforcing() rejects Null at registry level; release-gate
             negative test NEG:TRUST:NULL_PROVIDER_FORBIDDEN_IN_RELEASE proves
             the rejection occurs.

Threat T-32: A stale ProviderHandle is used after the provider was replaced
             (potentially with a less-privileged or null replacement).
Mitigation:  per-slot generation counter; lookup() returns StaleHandle.

Threat T-33: A sealed key for one purpose is misused as another.
Mitigation:  SealedKey carries `purpose`; unseal_key checks it before MAC
             verification (PurposeMismatch error).
```

### 8.2 Default-deny posture

The registry exposes no ambient authority. Calling `lookup()` with an unset
or unregistered handle returns `NotFound`. Calling provider methods through
the trait requires having obtained a reference to the provider in the first
place, which only happens via IPC mediated by `cap-broker`.

### 8.3 Audit emission

Every state transition emits an audit event:

```text
TrustProviderRegistered   { id, kind, profile, gen }
TrustProviderReplaced     { id, old_gen, new_gen, new_kind }
TrustProviderFaulted      { id, gen, reason_code }
TrustProviderWithdrawn    { id, gen }
TrustProviderRegistryEnforcing { providers_registered }
```

Events are appended through the existing v0.2 audit ring; no new ring is
introduced.

---

## 9. Memory / Resource Design

- The registry is a single fixed-size array of 8 slots (`MAX_PROVIDERS = 8`).
  Total size: 8 * (descriptor 28 B + generation 2 B + occupied 1 B + pad) =
  ≈ 256 B. Suitable for static allocation in `verifyd`.
- `KeyMaterial` is 65 B (`[u8; 64] + u8`). `SealedKey` is 102 B. Both stack-only.
- No `alloc`, no `Box<dyn HardwareTrustProvider>` in the registry — the
  registry stores descriptors only; provider trait objects live in the owning
  service alongside their static `RefCell` storage.

---

## 10. Compatibility and Migration

### 10.1 Compatibility with v0.2

- `verifyd`'s existing IPC protocol is preserved. The signature-checking
  internals move behind the trait but the wire format and tags do not change.
- `attestd`'s record schema (RFC v0.3-004) **does** change — see that RFC.
  The data model in this RFC adds no new external types.

### 10.2 Migration plan

| Step | Action                                                        | Risk     |
|------|---------------------------------------------------------------|----------|
| 1    | Land `fjell-trust-provider` crate (this RFC) with host tests. | None     |
| 2    | Land `verifyd` refactor that swaps internal sig code for the trait. | Low (refactor only) |
| 3    | Add registry to `attestd`, `measuredd`, `upgraded` as a *read-only consumer*. | None |
| 4    | RFC v0.3-002 — keyring & signature provider land on top.       | Tracked separately |

Migration is staged across patch releases (`v0.3.0-alpha.1` → `v0.3.0-rc.1` → `v0.3.0`).

---

## 11. Test Strategy

### 11.1 Host unit tests (this RFC)

In `crates/fjell-trust-provider/src/tests.rs`:

```text
- provider_id_unset_is_sentinel
- provider_handle_default_is_unset
- key_purpose_tags_are_stable             (on-wire stability)
- key_purpose_verification_only_classification
- descriptor_permitted_in_release_excludes_null
- capabilities_contains_and_union
- registry_starts_in_bootstrap
- registry_register_assigns_increasing_ids
- registry_lookup_returns_descriptor
- registry_lookup_rejects_unset_handle
- registry_register_full_returns_capacity_exhausted
- registry_enter_enforcing_is_one_way
- registry_enforcing_rejects_null_provider
- registry_enforcing_rejects_new_non_null_provider   (PhaseLocked)
- registry_replace_rotates_generation
- registry_replace_in_enforcing_rejects_null
- registry_remove_rotates_generation
- registry_stale_handle_after_replace_rejected
- registry_stale_handle_after_remove_rejected
- development_provider_signs_deterministically
- development_provider_sign_unsign_round_trip       (seal/unseal)
- development_provider_sign_wrong_purpose_rejected  (PurposeMismatch)
- development_provider_corrupted_blob_fails_mac     (SealIntegrityFailed)
- development_provider_rollback_counter_monotonic
- development_provider_faulted_rejects_all          (ProviderUnavailable)
- null_provider_returns_not_supported_everywhere
```

Target: ≥ 25 host tests.

### 11.2 QEMU negative tests

| Marker                                                       | Profile         |
|--------------------------------------------------------------|-----------------|
| `NEG:TRUST:NULL_PROVIDER_FORBIDDEN_IN_RELEASE`              | trust           |
| `NEG:TRUST:STALE_HANDLE_REJECTED`                            | trust           |
| `NEG:TRUST:FAULTED_PROVIDER_REJECTS_SIGN`                    | trust           |
| `NEG:TRUST:PURPOSE_MISMATCH_REJECTED`                        | trust           |
| `NEG:TRUST:SEAL_INTEGRITY_FAILURE_REJECTED`                  | trust           |

A new `trust` category in `crates/fjell-neg-test` and a matching CI matrix
row.

### 11.3 Property tests (deferred to v0.6 RFC 001)

```text
- "round-trip seal/unseal for any KeyPurpose, any KeyMaterial up to 64 B
   recovers the original bytes" — proptest in v0.6.
- "registry handle generation strictly monotonic under any operation sequence"
   — proptest in v0.6.
```

---

## 12. Acceptance Criteria

```text
- New crate `fjell-trust-provider` exists, builds on host, builds cross
  for riscv64gc-unknown-none-elf via build-std.
- Crate has ≥ 25 passing host unit tests.
- verifyd compiles against the crate and the existing `release-verify` smoke
  passes.
- ProviderRegistry's enter_enforcing transition is provably one-way (test
  registry_enter_enforcing_is_one_way passes).
- Null provider rejection in enforcing phase is proved by the QEMU marker
  NEG:TRUST:NULL_PROVIDER_FORBIDDEN_IN_RELEASE.
- Provider state changes appear in the semantic stream.
- Audit ring receives TrustProviderRegistered events.
- New ADR ADR-v0.3-001 (this RFC) is filed under docs/src/adr/.
```

---

## 13. Documentation Requirements

```text
docs/src/architecture/v0.3-001-trust-provider.md      — overall architecture
docs/src/development/v0.3-001-trust-provider.md       — implementer notes
docs/src/verification/v0.3-001-trust-provider-invariants.md
docs/src/adr/v0.3-001-trust-provider-boundary.md      — security-boundary ADR
```

CHANGELOG entries for each landed crate version.

---

## 14. Open Questions

1. **TPM/DICE descriptor name length** — 8 bytes covers `fjell-tp`, `fjell-dc`
   but production vendors may want a longer name (e.g. NIST CMVP module
   identifier). Recommendation: revisit when a real TPM provider is drafted in
   v0.3.x; do not enlarge the descriptor speculatively.
2. **Per-provider rollback counters vs system-wide** — current design is
   per-provider (each provider exposes its own counter). For TPM 2.0 there is
   a system-wide NV counter, so a TPM provider may report the same counter for
   all callers. This is acceptable; the trait does not promise per-purpose
   counters.
3. **Async-safety** — the trait is synchronous. For TPM operations that take
   100s of ms, this blocks the calling service. v0.4 introduces user-space
   timer infrastructure that may be used to wrap synchronous trait calls in a
   cooperative loop. Tracked in v0.4 RFC 002.

---

## 15. Release Gate (RFC-local)

This RFC is *Implemented* when:

```text
- Code merged to main on a branch tagged v0.3.0-alpha.1 or later.
- All host unit tests pass.
- At least one QEMU negative-test marker
  (NEG:TRUST:NULL_PROVIDER_FORBIDDEN_IN_RELEASE) is green in CI.
- CHANGELOG entry filed.
- ADR-v0.3-001 status set to Accepted.
```
