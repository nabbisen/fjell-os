//! Lease-based capability delegation types.
//!
//! A `LeaseId` is an opaque handle to a kernel-managed lease object.  The
//! kernel tracks a monotonic `epoch` per lease; capabilities bound to a
//! lease carry the epoch at which they were issued.  When the lease is
//! revoked (`LeaseRevoke` syscall), the epoch is incremented and all
//! previously issued bound capabilities fail future checks.

/// Opaque lease identifier.
///
/// Packed into a `u32` for ABI efficiency: upper 16 bits = generation
/// (prevents handle reuse), lower 16 bits = slot index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct LeaseId(pub u32);

impl LeaseId {
    pub const INVALID: LeaseId = LeaseId(u32::MAX);

    #[inline]
    pub fn new(index: u16, generation: u16) -> Self {
        LeaseId(((generation as u32) << 16) | (index as u32))
    }

    #[inline]
    pub fn index(self) -> u16 {
        (self.0 & 0xFFFF) as u16
    }

    #[inline]
    pub fn generation(self) -> u16 {
        (self.0 >> 16) as u16
    }

    #[inline]
    pub fn is_valid(self) -> bool {
        self != Self::INVALID
    }
}

/// Packed lease epoch checked during capability validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct LeaseEpoch(pub u32);

// ── Pure lease-decision logic (RFC-v0.17-003) ──────────────────────────────────
//
// These are the pure predicates the kernel lease table implements
// (crates/fjell-kernel/src/lease/mod.rs). They are extracted here, in a
// host-testable crate, so the Verus model
// (verification/verus/lease/lease_epoch.rs) and the kernel share one
// source of truth that ordinary `cargo test` can exercise.
//
// LEASE-VERUS-001: a binding is usable iff the lease is active and the
//                  current epoch equals the epoch recorded at issue.
// LEASE-VERUS-002: revoke increments the epoch.

/// Is a capability bound at `epoch_at_issue` usable against a lease that is
/// `active` with `current_epoch`?  Mirrors `LeaseTable::check_active`.
#[inline]
pub fn lease_usable(active: bool, current_epoch: u32, epoch_at_issue: u32) -> bool {
    active && current_epoch == epoch_at_issue
}

/// Outcome of a bounded lease revoke (architect condition C6, RFC-v0.17-003).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RevokeOutcome {
    /// Epoch advanced to the contained value (`old + 1`).
    Advanced(u32),
    /// Epoch counter exhausted (`u32::MAX`): the lease MUST be retired,
    /// never wrapped.
    MustRetire,
}

/// Bounded revoke: at `u32::MAX` the lease MUST be retired, never wrapped,
/// preserving strict epoch monotonicity (LEASE-VERUS bounded invariant).
///
/// This is the retire-before-wrap policy: the Verus model
/// (verification/verus/lease/lease_epoch.rs) proves strict monotonicity over
/// unbounded `nat` with the precondition `epoch < u32::MAX`; this function is
/// the u32 mirror of that bounded domain, and `MustRetire` is the kernel's
/// obligation at the boundary. Mirrors `LeaseTable::revoke`.
/// (Unreachable in practice: requires 2^32 revocations of one slot.)
#[inline]
pub fn lease_revoke(current_epoch: u32) -> RevokeOutcome {
    if current_epoch == u32::MAX {
        RevokeOutcome::MustRetire
    } else {
        RevokeOutcome::Advanced(current_epoch + 1)
    }
}

// ── Slot reuse (RFC-0.46-001) ─────────────────────────────────────────────────
//
// Until 0.46 no lease slot was ever reused, so "epochs are monotone within a slot" was the whole
// safety argument. Reuse deletes the second half of that sentence; identity across incarnations is the
// **generation's** job. These are the pure decisions the kernel lease table makes about reuse, mirrored
// by `verification/verus/lease/lease_epoch.rs` (`recycle`, `can_recycle`, `usable` over `(gen, epoch)`)
// and driven by `crates/fjell-cap/tests/lease_conformance.rs`.

/// The lifecycle a slot is in, as the allocator sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotKind {
    Empty,
    Active,
    Revoked,
}

/// What the allocator needs to know about one slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotView {
    pub kind: SlotKind,
    pub generation: u16,
    pub epoch: u32,
}

/// May a slot be reclaimed? Only a `Revoked` one, and only while **both** counters have headroom:
/// a slot at `generation == u16::MAX` or `epoch == u32::MAX` is retired for good (retire-before-wrap,
/// as LEASE-VERUS-005 already requires of the epoch), because a wrapped generation would make a
/// binding issued 65 536 incarnations ago match the current one.
#[inline]
pub fn lease_can_recycle(kind: SlotKind, generation: u16, epoch: u32) -> bool {
    kind == SlotKind::Revoked && generation < u16::MAX && epoch < u32::MAX
}

/// Where the next lease goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alloc {
    /// A never-used slot (lowest index).
    Fresh(usize),
    /// A revoked slot, reclaimed: the caller bumps its generation.
    Recycle(usize),
    /// Neither exists: the table is full.
    Full,
}

/// Choose a slot: an `Empty` one first (lowest index), otherwise the first reclaimable `Revoked`
/// one **at or after `cursor`, wrapping** -- round-robin, so the longest-revoked slot goes first and a
/// stale binding keeps reading `LeaseRevoked` for as long as possible.
pub fn lease_alloc(slots: &[SlotView], cursor: usize) -> Alloc {
    if let Some(i) = slots.iter().position(|s| s.kind == SlotKind::Empty) {
        return Alloc::Fresh(i);
    }
    let n = slots.len();
    for off in 0..n {
        let i = (cursor + off) % n;
        let s = slots[i];
        if lease_can_recycle(s.kind, s.generation, s.epoch) {
            return Alloc::Recycle(i);
        }
    }
    Alloc::Full
}

/// Per-owner bound (RFC-0.46-001 R-Q): an owner holding `owned_active` `Active` leases may create
/// another only while that is below `limit`.
#[inline]
pub fn lease_quota_ok(owned_active: usize, limit: usize) -> bool {
    owned_active < limit
}

/// How a `LeaseId`'s generation relates to the slot it indexes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdRelation {
    /// The id names the slot's current incarnation.
    Current,
    /// The id names an **earlier** incarnation: the lease it was issued for no longer exists.
    Stale,
    /// The id names an incarnation that has not happened: never issued.
    Forged,
}

#[inline]
pub fn lease_id_relation(slot_generation: u16, id_generation: u16) -> IdRelation {
    use core::cmp::Ordering::*;
    match id_generation.cmp(&slot_generation) {
        Equal => IdRelation::Current,
        Less => IdRelation::Stale,
        Greater => IdRelation::Forged,
    }
}

/// Is a binding issued at `(id_generation, epoch_at_issue)` usable against the slot's current
/// incarnation? The generalisation of [`lease_usable`] the Verus model proves: it additionally
/// requires the **same incarnation**, which the epoch alone cannot establish once a slot is reused.
#[inline]
pub fn lease_usable_in_incarnation(
    active: bool,
    slot_generation: u16,
    current_epoch: u32,
    id_generation: u16,
    epoch_at_issue: u32,
) -> bool {
    lease_id_relation(slot_generation, id_generation) == IdRelation::Current
        && lease_usable(active, current_epoch, epoch_at_issue)
}

#[cfg(test)]
mod reuse_tests {
    use super::*;

    fn v(kind: SlotKind, generation: u16, epoch: u32) -> SlotView {
        SlotView {
            kind,
            generation,
            epoch,
        }
    }

    #[test]
    fn an_empty_slot_is_taken_before_any_revoked_one() {
        let s = [
            v(SlotKind::Revoked, 0, 2),
            v(SlotKind::Empty, 0, 0),
            v(SlotKind::Active, 0, 1),
        ];
        assert_eq!(lease_alloc(&s, 0), Alloc::Fresh(1));
    }

    #[test]
    fn with_no_empty_slot_the_revoked_one_at_the_cursor_is_reclaimed_round_robin() {
        let s = [
            v(SlotKind::Revoked, 0, 2),
            v(SlotKind::Active, 0, 1),
            v(SlotKind::Revoked, 3, 9),
            v(SlotKind::Active, 0, 1),
        ];
        assert_eq!(lease_alloc(&s, 0), Alloc::Recycle(0));
        assert_eq!(
            lease_alloc(&s, 1),
            Alloc::Recycle(2),
            "from the cursor, not from the start"
        );
        assert_eq!(lease_alloc(&s, 3), Alloc::Recycle(0), "and wrapping");
    }

    #[test]
    fn a_table_of_active_leases_is_full() {
        let s = [v(SlotKind::Active, 0, 1); 4];
        assert_eq!(lease_alloc(&s, 0), Alloc::Full);
    }

    #[test]
    fn a_slot_at_the_generation_or_epoch_ceiling_is_retired_not_reclaimed() {
        assert!(lease_can_recycle(SlotKind::Revoked, u16::MAX - 1, 5));
        assert!(
            !lease_can_recycle(SlotKind::Revoked, u16::MAX, 5),
            "generation retire-before-wrap"
        );
        assert!(
            !lease_can_recycle(SlotKind::Revoked, 0, u32::MAX),
            "epoch retire-before-wrap"
        );
        let s = [
            v(SlotKind::Revoked, u16::MAX, 5),
            v(SlotKind::Revoked, 0, u32::MAX),
        ];
        assert_eq!(lease_alloc(&s, 0), Alloc::Full, "both are retired");
    }

    #[test]
    fn only_a_revoked_slot_is_ever_reclaimed() {
        assert!(
            !lease_can_recycle(SlotKind::Active, 0, 1),
            "never under its holder"
        );
        assert!(!lease_can_recycle(SlotKind::Empty, 0, 0));
    }

    #[test]
    fn the_quota_is_a_strict_bound() {
        assert!(lease_quota_ok(15, 16));
        assert!(!lease_quota_ok(16, 16));
        assert!(!lease_quota_ok(17, 16));
    }

    #[test]
    fn a_stale_id_is_stale_and_a_future_id_is_forged() {
        assert_eq!(lease_id_relation(3, 3), IdRelation::Current);
        assert_eq!(lease_id_relation(3, 2), IdRelation::Stale);
        assert_eq!(lease_id_relation(3, 4), IdRelation::Forged);
    }

    #[test]
    fn a_binding_from_an_earlier_incarnation_is_not_usable_whatever_the_epoch() {
        // Revoke then reuse; the new incarnation's epoch is chosen to COLLIDE with the old binding's.
        // The epoch alone would accept it; the generation does not.
        let (old_gen, old_epoch) = (4u16, 7u32);
        let new_gen = old_gen + 1;
        assert!(
            lease_usable(true, old_epoch, old_epoch),
            "control: the epoch alone is fooled"
        );
        assert!(!lease_usable_in_incarnation(
            true, new_gen, old_epoch, old_gen, old_epoch
        ));
        assert!(
            lease_usable_in_incarnation(true, old_gen, old_epoch, old_gen, old_epoch),
            "control: same incarnation"
        );
    }
}
