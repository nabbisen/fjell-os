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
//   crates/fjell-abi/src/lease_logic.rs :: lease_usable / revoke_epoch
// and the conformance test drives that mirror over the same cases.
//
// STATUS: machine-checked (v0.17.1); see verification/verus/TOOLCHAIN.lock.
//
// ASSUMPTIONS:
//   A1. `epoch` is a monotonic counter; revoke increments it by one.
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
    requires usable(lease, binding),
    ensures !usable(revoke(lease), binding),
{
    // revoke sets active=false, so usable() short-circuits to false.
}

// A binding minted at the NEW epoch is also not usable, because revoke
// leaves the lease inactive — re-issue requires a fresh active lease.
proof fn revoke_blocks_even_new_epoch_binding(lease: Lease)
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
// This is what makes stale bindings permanently unusable (no wraparound in
// the model; the kernel uses wrapping_add on u32 — see conformance note).
proof fn revoke_advances_epoch(lease: Lease)
    ensures revoke(lease).epoch > lease.epoch,
{
}

} // verus!
