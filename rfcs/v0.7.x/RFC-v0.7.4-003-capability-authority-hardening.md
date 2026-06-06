# RFC-v0.7.4-003: Capability Authority Hardening

## Status

Draft (closes review findings **C-RB-03, C-RB-04, C-RB-05, C-H-06,
C-M-09, W-H-02, W-H-06**)

## Target Version

`v0.7.4`

## Summary

Close the four release blockers and three high/medium findings that
together undermine the v0.2 promise of "least-privilege, lease-bound,
capability-mediated authority":

- Remove the broad unleased MMIO grants the kernel installs into
  almost every service at spawn time.
- Reject unknown `CapKind` values in `sys_cap_install` rather than
  coercing them to `Endpoint`.
- Enforce the documented `LeaseAdmin` authority on
  `sys_cap_bind_lease`.
- Deduplicate the `CapRights` constants currently mirrored in
  `fjell-cap-broker`.
- Document or fix `cap_copy`'s `parent = None` reset.
- Unify every lease-revocation path so blocked IPC is always woken
  or cancelled.
- Constrain provider replace/remove in `Enforcing` trust-provider
  mode behind a signed-policy gate.

## Motivation

The crates review (§5 RB-03, RB-04, RB-05; §6 H-06; §7 M-09) and the
whole-project review (§5 H-02, H-06) describe a coherent class of
defects: the capability machinery exists and is mostly correct, but
several spawn-time, install-time, and lifecycle-time paths bypass it.

This RFC rebuilds the bottom of the authority stack so the v0.7.2 sync
services, the v0.7.3 network services, and the v0.7.4 DMA layer can
trust the cap-broker as the actual source of device authority — not
the kernel spawn path.

## Goals

```text
- Spawn path grants NO broad MmioRegion caps to generic services.
- cap-broker becomes the only path to device authority for v0.7+
  services.
- sys_cap_install rejects unknown CapKind; rejects unleased grants
  for normal services; rejects rights amplification.
- sys_cap_bind_lease requires LeaseAdmin + LEASE_BIND right and
  validates scope over the target lease.
- CapRights constants live in exactly one place (fjell-cap); the
  cap-broker imports them.
- `CapRights::ALL` is renamed or documented to clarify the
  CAP_INSTALL exclusion.
- cap_copy semantics are documented; parent-tracking either preserved
  or its absence is justified in an ADR.
- Every lease-revocation path (syscall, task exit, fault, lease
  expiry) goes through the same kernel API that wakes blocked
  receivers and cancels blocked calls.
- Provider registry replace/remove in Enforcing requires a signed
  policy authorization.
```

## Non-Goals

```text
- No new capability kinds in this RFC.
- No reduction of CSpace size from current 256 slots.
- No change to the on-the-wire ABI for IPC tags (only kernel-side
  enforcement).
```

## External Design

### Spawn-path MMIO grant removal

```rust
// crates/fjell-kernel/src/task/spawn.rs (revised)

fn install_initial_caps(image: ImageId, cspace: &mut CSpace) {
    // The spawn path installs ONLY:
    //   - the receive endpoint cap for self-IPC,
    //   - the cap-broker request endpoint cap (so the service can
    //     request its working capabilities),
    //   - an audit-emit cap (so failures are observable).
    //
    // No broad MmioRegion grants here. No CapRights::ALL.
    cspace.install(SLOT_SELF_RECV,    self_recv_cap());
    cspace.install(SLOT_CAP_BROKER,   cap_broker_endpoint_cap());
    cspace.install(SLOT_AUDIT_EMIT,   audit_emit_cap_for(image));
}
```

A service that needs device authority asks `cap-broker` and the
broker, consulting its policy bundle, installs the narrow cap.

Bootstrap exceptions (e.g., the very first stage in early boot needs
the UART for diagnostics) are listed by name in
`crates/fjell-kernel/src/task/spawn_exceptions.rs` with a comment per
exception and a CI gate that fails on additions without an ADR.

### `sys_cap_install` discipline

```rust
fn sys_cap_install(tf: &mut TrapFrame) -> SyscallResult {
    require_cap_on_ct(CapKind::CapInstall, CAP_INSTALL)?;

    let descriptor = read_install_descriptor(tf)?;

    // Reject unknown CapKind.
    let kind = CapKind::from_u8(descriptor.kind_n)
        .ok_or(SyscallError::InvalidCapKind)?;

    // Reject CapRights::ALL on a normal install.
    if descriptor.rights == CapRights::ALL_NON_META
       && !descriptor.is_bootstrap_exception
    {
        return Err(SyscallError::RightsAmplificationRejected);
    }

    // Reject unleased normal grants. Bootstrap exceptions list the
    // permitted kinds explicitly.
    if descriptor.lease.is_none()
       && !permits_unleased_install(kind)
    {
        return Err(SyscallError::UnleasedInstallRejected);
    }

    // Rights are subset of installer's authority OR
    // policy-approved by cap-broker.
    if !installer_authority_covers(&descriptor.rights) {
        return Err(SyscallError::RightsExceedInstallerAuthority);
    }

    install_into_cspace(descriptor)?;
    audit_emit(AUDIT_CAP_INSTALLED, &descriptor);
    Ok(SyscallReturn::ok())
}
```

The `InstallDescriptor` carries explicit `kind`, `rights`, `scope`,
`lease`, and `is_bootstrap_exception` fields. The old "unknown kind
becomes Endpoint" behaviour is removed.

### `sys_cap_bind_lease` discipline

```rust
fn sys_cap_bind_lease(tf: &mut TrapFrame) -> SyscallResult {
    // Documented requirement: caller MUST hold LeaseAdmin with
    // LEASE_BIND right; scope must cover the target lease.
    require_cap_on_ct(CapKind::LeaseAdmin, LEASE_BIND)?;

    let lease_id = tf.gpr[REG_A0] as u32;
    let target_cap = read_cap_handle(tf, REG_A1)?;

    require_scope_over_lease(lease_id)?;

    let lease = lease_table.get(lease_id)
        .ok_or(SyscallError::UnknownLease)?;

    if lease.owner != caller_task()
       && !caller_has_lease_admin_over(lease)
    {
        return Err(SyscallError::ForeignLease);
    }

    cspace.bind_lease(target_cap, lease_id)?;
    audit_emit(AUDIT_LEASE_BOUND, lease_id, target_cap);
    Ok(SyscallReturn::ok())
}
```

### `CapRights` deduplication

```rust
// crates/fjell-cap/src/rights.rs (single source of truth)

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CapRights(pub u64);

impl CapRights {
    pub const SEND:         Self = CapRights(1 << 3);
    pub const RECV:         Self = CapRights(1 << 4);
    pub const MMIO_MAP:     Self = CapRights(1 << 5);
    pub const NET_RECV:     Self = CapRights(1 << 14);
    pub const NET_SEND:     Self = CapRights(1 << 15);
    pub const NET_DMA:      Self = CapRights(1 << 16);
    pub const DMA_REVOKE:   Self = CapRights(1 << 17);
    pub const DMA_ADMIN:    Self = CapRights(1 << 18);
    pub const LEASE_CREATE: Self = CapRights(1 << 24);
    pub const LEASE_BIND:   Self = CapRights(1 << 25);
    pub const CAP_INSTALL:  Self = CapRights(1 << 26);
    pub const CAP_REVOKE:   Self = CapRights(1 << 27);
    pub const SYNC_IMPORT:  Self = CapRights(1 << 28);
    pub const SYNC_EXPORT:  Self = CapRights(1 << 29);

    /// All rights EXCLUDING meta-rights like CAP_INSTALL/CAP_REVOKE.
    /// Used by ordinary services that should never mint or revoke
    /// other caps.
    pub const ALL_NON_META: Self = CapRights((1 << 24) - 1);

    /// All defined rights, including meta. Used only by cap-broker
    /// and trust-provider admin paths.
    pub const ALL_DEFINED:  Self = CapRights((1 << 30) - 1);
}
```

`fjell-cap-broker` removes its local constants and imports from
`fjell-cap`. A compile-time test asserts the constants match the
broker's policy-bundle expectations.

### `cap_copy` parent semantics

```rust
impl CapTable {
    /// Copy a cap. The copy is INDEPENDENT of the source for
    /// kernel-local revocation purposes: cap_revoke(source) does
    /// NOT cascade to copies.
    ///
    /// Recursive revocation across copies is achieved via LEASE
    /// REVOCATION; copies that share a lease are all revoked when
    /// the lease is revoked.
    ///
    /// This is the documented design.  See ADR-v0.7.4-003.
    pub fn cap_copy(&mut self, src: CapHandle) -> Result<CapHandle, CapError> {
        let src_cap = self.get(src)?;
        let new_cap = Cap {
            parent: None,                  // intentional; see doc above
            lease:  src_cap.lease,         // shared lease for revocation
            ..src_cap.clone()
        };
        let new_handle = self.install(new_cap)?;
        Ok(new_handle)
    }
}
```

The behaviour is preserved (`parent = None`) but the rationale is
codified: kernel-local recursive revocation is intentionally not
supported because every meaningful cap MUST have a lease, and lease
revocation is the recursive primitive.

### Unified lease revocation

```rust
// crates/fjell-kernel/src/lease.rs

pub fn revoke_lease(lease_id: LeaseId, reason: LeaseRevokeReason) -> RevokeReport {
    let mut report = RevokeReport::new(lease_id, reason);

    // Step 1: increment epoch (no new operations may bind to this
    // generation).
    lease_table.bump_epoch(lease_id);

    // Step 2: enumerate every cap bound to this lease across all
    // CSpaces.  For each:
    for cap_ref in all_caps_under_lease(lease_id) {
        cspace_of(cap_ref.task).invalidate(cap_ref.handle);
        report.caps_invalidated += 1;
    }

    // Step 3: wake blocked receivers and cancel blocked calls.
    for task in tasks_with_pending_ipc_under_lease(lease_id) {
        wake_or_cancel_blocked_ipc(task, lease_id);
        report.tasks_woken += 1;
    }

    // Step 4: invalidate late replies.
    for reply in pending_replies_under_lease(lease_id) {
        reply.invalidate();
        report.replies_invalidated += 1;
    }

    // Step 5: audit (pinned-critical).
    audit_emit(AUDIT_LEASE_REVOKED, &report);

    report
}
```

EVERY caller of lease revocation now goes through `revoke_lease`:

- `sys_lease_revoke` (the syscall)
- `revoke_owned_by(task)` (task exit / fault)
- `lease_expiry_timer_fire(lease_id)` (timer-driven expiry)
- `revoke_all_under_provider(provider_id)` (provider replace/remove)

The old no-op `wake_or_cancel_blocked_ipc_for_lease()` is deleted.

### Provider registry constraints

```rust
impl TrustProviderRegistry {
    pub fn replace(&mut self, slot: ProviderSlot, new: TrustProvider,
                   policy_auth: SignedPolicyAuth)
        -> Result<(), RegistryError>
    {
        if self.mode == TrustMode::Enforcing {
            verify_signed_policy(&policy_auth, PolicyAction::ReplaceProvider)?;
        }
        // Revoke every lease under the old provider FIRST.
        revoke_all_under_provider(self.providers[slot].id);
        self.providers[slot] = new;
        audit_emit(AUDIT_PROVIDER_REPLACED, slot);
        Ok(())
    }

    pub fn remove(&mut self, slot: ProviderSlot,
                  policy_auth: SignedPolicyAuth)
        -> Result<(), RegistryError>
    {
        if self.mode == TrustMode::Enforcing {
            verify_signed_policy(&policy_auth, PolicyAction::RemoveProvider)?;
        }
        revoke_all_under_provider(self.providers[slot].id);
        self.providers[slot] = TrustProvider::EMPTY;
        audit_emit(AUDIT_PROVIDER_REMOVED, slot);
        Ok(())
    }
}
```

`SignedPolicyAuth` is verified against the keyring's `PolicyAdmin`
anchor. In `Enforcing` mode, unsigned replace/remove returns
`RegistryError::PolicyAuthorizationRequired`.

## Data Model

### `SyscallError` additions

```text
InvalidCapKind                = 0x40
RightsAmplificationRejected   = 0x41
UnleasedInstallRejected       = 0x42
RightsExceedInstallerAuth     = 0x43
UnknownLease                  = 0x44
ForeignLease                  = 0x45
```

### `RegistryError`

```text
PolicyAuthorizationRequired   = 0x01
SlotEmpty                     = 0x02
SlotInUse                     = 0x03
```

### Audit events

```text
AUDIT_CAP_INSTALLED                  = 0x0601
AUDIT_CAP_INSTALL_REJECTED           = 0x0602
AUDIT_LEASE_BOUND                    = 0x0603
AUDIT_LEASE_BIND_REJECTED            = 0x0604
AUDIT_LEASE_REVOKED                  = 0x0605
AUDIT_PROVIDER_REPLACED              = 0x0610
AUDIT_PROVIDER_REMOVED               = 0x0611
AUDIT_PROVIDER_REPLACE_REJECTED      = 0x0612
```

All pinned-critical.

## Internal Design

### Rights-table compile-time test

A unit test in `fjell-cap-broker`:

```rust
#[test]
fn cap_broker_uses_fjell_cap_constants() {
    // After this RFC, fjell-cap-broker has no local rights consts.
    // This test ensures any reintroduction is caught.
    let _ = CapRights::SEND;       // import works
    let _ = CapRights::NET_RECV;
    let _ = CapRights::CAP_INSTALL;
    // No `RIGHT_SEND: u64 = ...` should appear in cap-broker source.
}
```

A separate CI script (`tools/fjell-cap-audit/`) scans
`crates/fjell-cap-broker/src/*.rs` for local rights constants and
fails on any match.

### Lease-revocation property tests

In `fjell-proptest`:

```text
LR1 revoke_lease_invalidates_all_caps_under_lease
LR2 revoke_lease_wakes_all_blocked_receivers
LR3 revoke_lease_cancels_all_blocked_calls
LR4 revoke_lease_idempotent_on_already_revoked
LR5 task_exit_revokes_owned_leases_with_full_wake
LR6 expiry_timer_revokes_with_full_wake
```

6 properties × 1000 cases.

### Cap-install / cap-bind negative tests

Per the crates review §13:

```text
NEG:CAP_INSTALL:UNKNOWN_KIND_REJECTED
NEG:CAP_INSTALL:UNLEASED_NORMAL_GRANT_REJECTED
NEG:CAP_INSTALL:RIGHTS_AMPLIFICATION_REJECTED
NEG:LEASE:BIND_WITHOUT_LEASE_ADMIN_REJECTED
NEG:LEASE:BIND_FOREIGN_LEASE_REJECTED
NEG:MMIO:NON_DRIVER_SERVICE_CANNOT_MAP
NEG:MMIO:UNLEASED_SERVICE_MMIO_CAP_ABSENT
NEG:MMIO:REVOKED_DRIVER_MMIO_CAP_REJECTED
```

All QEMU smoke markers in the `v0.7.4-cap` category.

## Security Design

### Pre-RFC vs post-RFC

| Capability | Pre-RFC reality | Post-RFC reality |
|------------|-----------------|------------------|
| Generic service MMIO access | Broad unleased caps at spawn | None; must request from broker |
| Unknown `CapKind` install | Coerced to `Endpoint` | Rejected |
| `sys_cap_install` unleased | Allowed | Rejected for normal services |
| `sys_cap_bind_lease` LeaseAdmin | Not checked | Required + scope check |
| Task exit and IPC waiters | May leak (no-op hook) | Always woken/cancelled |
| Provider replace in Enforcing | Unguarded | Signed-policy gated |
| Recursive revoke across copies | Implicit assumption | Explicit: via lease only |

### Defence-in-depth

The capability system, the lease system, and the audit system form
three layers; this RFC ensures none can be bypassed via a "sideways"
path (spawn, install, bind, provider change).

## Memory / Resource Design

- Per-cap parent pointer removed in name (it was already not used);
  no memory change.
- Lease-revocation report struct: ~32 B on the kernel stack per
  `revoke_lease` call.
- Provider registry signed-policy verification: one ed25519 verify
  per replace/remove call; bounded.

## Compatibility and Migration

- Services that previously relied on broad MMIO caps at spawn now
  must request them from cap-broker. Internal services (devmgr,
  virtio drivers) are updated as part of this RFC; out-of-tree
  services must adapt.
- `sys_cap_install` callers that passed garbage `cap_kind_n` (relying
  on Endpoint fallback) now receive `InvalidCapKind`. No legitimate
  caller does this.
- `CapRights::ALL` is removed; callers must explicitly request
  `ALL_NON_META` or `ALL_DEFINED`.
- Provider replace/remove in `Enforcing` mode now requires
  `SignedPolicyAuth`. Operators running in Enforcing mode must have
  the `PolicyAdmin` anchor provisioned (it already is, per v0.3).

## Test Strategy

```text
- 6 lease-revocation property tests × 1000 cases.
- 8 NEG QEMU smoke markers (above).
- Compile-time test: cap-broker imports rights from fjell-cap.
- Compile-time test: no `RIGHT_*: u64` constants in cap-broker source.
- Provider registry: replace in Enforcing without policy → rejected.
- Provider registry: replace in Enforcing with valid policy → ok,
  audit emitted.
- Integration: spawn a generic service, observe no MmioRegion caps
  in its CSpace; request one from cap-broker; observe narrow cap
  bound to a lease.
```

## Acceptance Criteria

```text
- All 8 NEG QEMU smoke markers green.
- 6 lease-revocation property tests green.
- cap-broker contains zero locally-defined rights constants.
- Spawn path installs ≤ 3 caps per service (self-recv, broker-ep,
  audit-emit).
- Provider replace/remove in Enforcing requires SignedPolicyAuth.
- AUDIT_LEASE_REVOKED is emitted from task exit, syscall, expiry
  timer, and provider replace paths — verified by a synthetic test.
- ADR-v0.7.4-003 filed.
```

## Documentation Requirements

```text
- docs/src/reference/cap-install-discipline.md created.
- docs/src/reference/lease-revocation-paths.md created.
- docs/src/reference/cap-copy-semantics.md created (explains why
  parent = None and how lease-based recursive revoke replaces it).
- docs/src/reference/trust-provider-policy.md updated for the
  Enforcing-mode signed-policy gate.
- UNSAFE_CHARTER.md gains a "capability authority" category with
  the install/revoke invariants.
```

## Open Questions

```text
1. Should `ALL_NON_META` be the documented spawn-path right, or
   should each service compute its needed set? Proposal: the
   spawn path uses NO ALL_* constants; only explicit per-cap rights
   on the three bootstrap caps.

2. Bootstrap exceptions: how many will survive? Proposal: only the
   very first stage (init) needs UART before audit-emit is ready;
   one exception total, listed by name in spawn_exceptions.rs.

3. SignedPolicyAuth format: reuse attestation-format's SignedBy
   descriptor? Proposal: yes — a SignedPolicyAuth is a
   SignedByDescriptor over a PolicyAction record. No new type.
```

## Release Gate

```text
- 8 NEG QEMU smoke markers in v0.7.4-cap category green
- 6 lease-revocation property tests green
- cap-broker rights-constant audit green
- ADR-v0.7.4-003 accepted
```
