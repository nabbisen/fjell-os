// Verus proof: lease epoch revocation.
// RFC-v0.17-003. Pilot target "lease".
//
// MAPS TO real Fjell code:
//   crates/fjell-kernel/src/lease/mod.rs
//     revoke():        slot.epoch += 1; slot.state = Revoked
//     check_active():  Active && slot.epoch == bound_epoch && id.generation == slot.generation
//     create():        reclaims a Revoked slot: generation += 1, state = Active (RFC-0.46-001)
//
// Because the kernel is no_std/bare-metal and not host-testable, the pure
// decision predicate is mirrored host-side in:
//   crates/fjell-abi/src/lease.rs :: lease_usable / lease_revoke
// and the conformance test drives that mirror over the same cases.
//
// STATUS: machine-checked (v0.17.1); see verification/verus/TOOLCHAIN.lock.
//
// RFC-0.46-001 (extended at 0.46): THIS MODEL NOW COVERS SLOT REUSE. Before 0.46 no slot was ever
// reused and the whole safety argument was "epochs are monotone within a slot that is never reused";
// reuse deletes the second half of that sentence, and what carries identity across incarnations is the
// GENERATION. `Lease`/`Binding` therefore carry `generation`, `usable` requires the binding's generation to be
// the slot's current one, and `recycle` is the new `Revoked -> Active` transition. The recycling
// obligations below do NOT consult the new incarnation's epoch -- they hold for any epoch the
// implementation chooses (the kernel continues it, but a restart would still be sound), which is the
// obligation "the epoch alone cannot carry once a slot restarts". Prototype evidence: with the generation
// clause removed from `usable` those obligations fail (3 errors).
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
//   A4. (0.46) `generation` is a u16 that NEVER wraps: a slot at u16::MAX is retired, not reclaimed
//       (`can_recycle`), the generation's retire-before-wrap, mirroring A1 for the epoch.

use vstd::prelude::*;

verus! {

pub struct Lease {
    pub active: bool,
    pub epoch:  nat,
    pub generation:    nat,
}

pub struct Binding {
    pub epoch_at_issue: nat,
    /// The incarnation (slot generation) the binding was issued against -- the high half of its LeaseId.
    pub generation_at_issue:   nat,
}

/// LEASE-VERUS-001: usable iff active, the same incarnation, and epoch matches the binding.
/// Mirrors check_active(): state==Active && epoch==bound_epoch && id.generation==slot.generation.
pub open spec fn usable(lease: Lease, binding: Binding) -> bool {
    lease.active && lease.generation == binding.generation_at_issue && lease.epoch == binding.epoch_at_issue
}

/// LEASE-VERUS-002: revoke deactivates and increments the epoch.
/// Mirrors revoke(): epoch += 1; state = Revoked.
pub open spec fn revoke(lease: Lease) -> Lease {
    Lease { active: false, epoch: lease.epoch + 1, generation: lease.generation }
}

/// LEASE-VERUS-006 (0.46): only a revoked slot, and only below the generation ceiling, may be reused.
/// Mirrors fjell_abi::lease::lease_can_recycle (which also retires at epoch == u32::MAX).
pub open spec fn can_recycle(lease: Lease) -> bool {
    !lease.active && lease.generation < u16::MAX as nat
}

/// LEASE-VERUS-007 (0.46): reuse advances the generation and starts a new active incarnation. The new
/// epoch is an ARBITRARY parameter on purpose: nothing proved below depends on it.
/// Mirrors LeaseTable::create's reclaim arm: generation += 1; state = Active.
pub open spec fn recycle(lease: Lease, new_epoch: nat) -> Lease {
    Lease { active: true, epoch: new_epoch, generation: lease.generation + 1 }
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
    ensures !usable(revoke(lease), Binding { epoch_at_issue: lease.epoch + 1, generation_at_issue: lease.generation }),
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

// ── Slot reuse (RFC-0.46-001) ─────────────────────────────────────────────────────────────────

// LEASE-VERUS-008: THE OBLIGATION THE EPOCH CANNOT CARRY. A binding usable before revoke is not
// usable against the reclaimed slot, for ANY epoch the new incarnation starts at.
proof fn recycled_slot_is_not_its_predecessor(l: Lease, b: Binding, new_epoch: nat)
    requires usable(l, b), can_recycle(revoke(l)),
    ensures !usable(recycle(revoke(l), new_epoch), b),
{
}

// ... including a new epoch chosen to COLLIDE with the old binding's (an epoch-only model resurrects it).
proof fn epoch_collision_does_not_resurrect(l: Lease, b: Binding, new_epoch: nat)
    requires usable(l, b), can_recycle(revoke(l)), b.epoch_at_issue == new_epoch,
    ensures !usable(recycle(revoke(l), new_epoch), b),
{
}

// LEASE-VERUS-009: generations are strictly monotone across reuse and stay representable in u16.
proof fn recycle_advances_generation(l: Lease, e: nat)
    requires can_recycle(l),
    ensures recycle(l, e).generation > l.generation, recycle(l, e).generation <= u16::MAX as nat,
{
}

// LEASE-VERUS-010: a live lease is never reused under its holder.
proof fn active_slot_cannot_be_recycled(l: Lease)
    requires l.active,
    ensures !can_recycle(l),
{
}

// LEASE-VERUS-011: no ABA at distance two -- a binding from incarnation g is dead at g + 2 as well.
proof fn no_aba_across_two_reuses(l: Lease, b: Binding, e1: nat, e2: nat)
    requires
        usable(l, b),
        can_recycle(revoke(l)),
        can_recycle(revoke(recycle(revoke(l), e1))),
    ensures !usable(recycle(revoke(recycle(revoke(l), e1)), e2), b),
{
}

// LEASE-VERUS-012: a binding issued against the NEW incarnation is usable until that one is revoked
// (reuse does not make a fresh lease unusable).
proof fn new_incarnation_binding_is_usable(l: Lease, new_epoch: nat)
    requires can_recycle(l),
    ensures usable(
        recycle(l, new_epoch),
        Binding { epoch_at_issue: new_epoch, generation_at_issue: l.generation + 1 },
    ),
{
}

} // verus!
