// Verus proof: lease epoch revocation.
// RFC-v0.17-003. Pilot target "lease".
//
// MAPS TO real Fjell code:
//   crates/fjell-kernel/src/lease/mod.rs
//     revoke():        slot.epoch += 1; slot.state = Revoked
//     check_active():  Active && slot.epoch == bound_epoch
//
// Because the kernel is no_std/bare-metal and not host-testable, the pure
// decision predicate is mirrored host-side in:
//   crates/fjell-abi/src/lease.rs :: lease_usable / lease_revoke
// and the conformance test drives that mirror over the same cases.
//
// STATUS: machine-checked (v0.17.1); see verification/verus/TOOLCHAIN.lock.
//
// ASSUMPTIONS:
//   A1. `epoch` is a monotonic counter; revoke increments it by one.
//       BOUNDED DOMAIN (architect C6, retire-before-wrap): the kernel epoch
//       is u32 and NEVER wraps. The proofs below carry the precondition
//       `epoch < u32::MAX`; at exactly u32::MAX the kernel retires the slot
//       (fjell_abi::lease::lease_revoke -> RevokeOutcome::MustRetire) instead
//       of advancing, so within the modeled domain kernel revoke == model
//       revoke and strict monotonicity holds without wraparound.
//   A2. A binding records the epoch at issue; it is usable only while the
//       lease is active AND the current epoch equals the issue epoch.
//   A3. cap_drop is modeled as always-permitted (LEASE-VERUS-004); it does
//       not consult epoch.

use vstd::prelude::*;

verus! {

pub struct Lease {
    pub active: bool,
    pub epoch:  nat,
}

pub struct Binding {
    pub epoch_at_issue: nat,
}

/// LEASE-VERUS-001: usable iff active and epoch matches the binding.
/// Mirrors check_active(): state==Active && epoch==bound_epoch.
pub open spec fn usable(lease: Lease, binding: Binding) -> bool {
    lease.active && lease.epoch == binding.epoch_at_issue
}

/// LEASE-VERUS-002: revoke deactivates and increments the epoch.
/// Mirrors revoke(): epoch += 1; state = Revoked.
pub open spec fn revoke(lease: Lease) -> Lease {
    Lease { active: false, epoch: lease.epoch + 1 }
}

/// cap_drop is always permitted (LEASE-VERUS-004): models drop ignoring epoch.
pub open spec fn drop_allowed(_lease: Lease) -> bool { true }

// LEASE-VERUS-003: a binding usable before revoke is not usable after.
proof fn revoked_binding_not_usable(lease: Lease, binding: Binding)
    requires
        usable(lease, binding),
        lease.epoch < u32::MAX as nat,  // bounded domain (C6)
    ensures !usable(revoke(lease), binding),
{
    // revoke sets active=false, so usable() short-circuits to false.
}

// A binding minted at the NEW epoch is also not usable, because revoke
// leaves the lease inactive — re-issue requires a fresh active lease.
proof fn revoke_blocks_even_new_epoch_binding(lease: Lease)
    requires lease.epoch < u32::MAX as nat,  // bounded domain (C6)
    ensures !usable(revoke(lease), Binding { epoch_at_issue: lease.epoch + 1 }),
{
    // revoke().active == false ⇒ usable == false regardless of epoch match.
}

// LEASE-VERUS-004: cap_drop remains allowed after revoke.
proof fn drop_allowed_after_revoke(lease: Lease)
    ensures drop_allowed(revoke(lease)),
{
}

// Monotonicity: the post-revoke epoch strictly exceeds the pre-revoke epoch.
// This is what makes stale bindings permanently unusable. The kernel enforces
// retire-before-wrap (C6): it never wraps, so this model matches the kernel
// for every reachable epoch.
proof fn revoke_advances_epoch(lease: Lease)
    requires lease.epoch < u32::MAX as nat,  // bounded domain (C6)
    ensures revoke(lease).epoch > lease.epoch,
{
}

// LEASE-VERUS-005 (C6): within the bounded domain the advanced epoch stays
// representable in u32 — the model revoke maps exactly onto
// fjell_abi::lease::lease_revoke's Advanced(old + 1) arm.
proof fn revoke_bounded_in_domain(lease: Lease)
    requires lease.epoch < u32::MAX as nat,
    ensures
        revoke(lease).epoch == lease.epoch + 1,
        revoke(lease).epoch <= u32::MAX as nat,
{
}

} // verus!
