//! The boot-control state machine (ADR-0009), on the block itself.
//!
//! RFC-0.33-001 D1. Until this module the block was a *format*: fields with a
//! checksum, touched at runtime by nothing but `init` writing a fresh one to
//! disk. The service that owns boot control (`fjell-bootctl`) held a
//! three-variant enum instead, and the model of the real transitions
//! (`fjell-bootctl-model`) had no dependents. This is the ADR's machine, in safe
//! code, on the type the ADR names — and `fjell-bootctl-model`'s tests check it
//! (see that crate's `tests/refines.rs`).
//!
//! # What a boot is
//!
//! **Every boot is a trial of the active slot.** [`begin_boot`] consumes one of
//! its `remaining_tries`; the system then either [`confirm`]s (health passed) or
//! [`fail_health`] and [`reboot`]s (health failed). The candidate mechanism is
//! the same machine one step earlier: [`set_candidate`] names the slot to try
//! next, and [`reboot`] switches to it.
//!
//! # What this does not do
//!
//! It selects **state**, not an image. There is one kernel image in this system,
//! so "the next boot's slot" is a field that nothing acts on (RFC-0.33-001 D7),
//! and nothing here is durable: writing the block to disk and reading it back
//! is a store client, which is a line of its own.
//!
//! Every transition is total — it returns an error rather than panicking, and a
//! refused transition changes **nothing** (each test that asserts a refusal
//! asserts the block is byte-for-byte unchanged).
//!
//! [`begin_boot`]: BootControlBlock::begin_boot
//! [`confirm`]: BootControlBlock::confirm
//! [`fail_health`]: BootControlBlock::fail_health
//! [`reboot`]: BootControlBlock::reboot
//! [`set_candidate`]: BootControlBlock::set_candidate

use crate::{BootControlBlock, NO_CANDIDATE, SlotId, SlotInfo, SlotState};

/// Why a transition was refused. A refusal leaves the block unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootError {
    /// A slot field holds a byte that is not a slot. Only a block that came
    /// from somewhere other than [`BootControlBlock::new`] can have one.
    InvalidSlot(u8),
    /// The slot holds no image, so it cannot be booted.
    SlotEmpty,
    /// The slot failed its health check or ran out of tries; it is not booted
    /// again until a new image is staged into it.
    SlotFailed,
    /// Confirming a slot that has not been booted — B1 of the model: a boot
    /// that did not happen cannot have been healthy.
    NotBooted,
    /// Staging into the slot the system is running. ADR-0009: an update is
    /// staged to the *inactive* slot; overwriting the running image is what
    /// `ACTIVE_SLOT_WRITE_REJECTED` is about.
    ActiveSlot,
    /// Staging into the last confirmed slot while another is running, which
    /// would destroy the only known-good image the system can fall back to.
    LastConfirmedSlot,
}

/// What a failed health check did to the block, and so what the caller may do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HealthOutcome {
    /// An unconfirmed slot failed and is now unbootable; a
    /// [`reboot`](BootControlBlock::reboot) will roll back to the last
    /// confirmed slot.
    CandidateFailed,
    /// The slot that failed *is* the last confirmed slot: nothing was marked and
    /// there is nothing to roll back to. **Do not reset on this.**
    NothingToRollBackTo,
}

/// What a health report means for the boot, and so what the service that owns
/// the block must do next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HealthVerdict {
    /// Healthy: the active slot is confirmed. Nothing further to do.
    Confirmed,
    /// An unconfirmed slot failed and is marked unbootable. The owner rolls
    /// back with [`reboot`](BootControlBlock::reboot) and resets the machine.
    MustRollBack,
    /// The slot that failed is the last confirmed one: the block is unchanged
    /// and **the owner must not reset** (see [`HealthOutcome`]).
    NoFallback,
}

impl SlotId {
    /// The other slot.
    pub const fn other(self) -> SlotId {
        match self {
            SlotId::A => SlotId::B,
            SlotId::B => SlotId::A,
        }
    }

    /// The slot a stored byte names, if it names one.
    pub const fn from_u8(v: u8) -> Option<SlotId> {
        match v {
            0 => Some(SlotId::A),
            1 => Some(SlotId::B),
            _ => None,
        }
    }
}

fn slot_of(v: u8) -> Result<SlotId, BootError> {
    SlotId::from_u8(v).ok_or(BootError::InvalidSlot(v))
}

impl SlotInfo {
    /// Whether this slot has been booted: confirmed, or at least one try
    /// spent. The block has no `booted_once` field; a try being consumed *is*
    /// the record that a boot began.
    pub const fn has_been_booted(&self) -> bool {
        self.confirmed == 1 || self.remaining_tries < self.tries_allowed
    }
}

impl BootControlBlock {
    /// The slot's record.
    pub fn slot(&self, id: SlotId) -> &SlotInfo {
        match id {
            SlotId::A => &self.slot_a,
            SlotId::B => &self.slot_b,
        }
    }

    fn slot_mut(&mut self, id: SlotId) -> &mut SlotInfo {
        match id {
            SlotId::A => &mut self.slot_a,
            SlotId::B => &mut self.slot_b,
        }
    }

    /// The slot the system is running.
    pub fn active(&self) -> Result<SlotId, BootError> {
        slot_of(self.active_slot)
    }

    /// The last slot that passed a health check — where a rollback goes.
    pub fn last_confirmed(&self) -> Result<SlotId, BootError> {
        slot_of(self.last_confirmed_slot)
    }

    /// The slot staged to boot next, if any.
    pub fn candidate(&self) -> Result<Option<SlotId>, BootError> {
        if self.candidate_slot == NO_CANDIDATE {
            Ok(None)
        } else {
            slot_of(self.candidate_slot).map(Some)
        }
    }

    /// CandidateSet: `slot` becomes the candidate for the next boot, holding
    /// an unconfirmed image of `image_generation`.
    ///
    /// A freshly staged image has not been booted and has not been confirmed,
    /// whatever the slot held before — so this clears the slot's confirmation,
    /// its spent tries and any earlier failure.
    pub fn set_candidate(&mut self, slot: SlotId, image_generation: u64) -> Result<(), BootError> {
        let active = self.active()?;
        let last_confirmed = self.last_confirmed()?;
        if slot == active {
            return Err(BootError::ActiveSlot);
        }
        if slot == last_confirmed {
            return Err(BootError::LastConfirmedSlot);
        }
        let allowed = self.slot(slot).tries_allowed;
        let s = self.slot_mut(slot);
        s.state = SlotState::Candidate;
        s.image_generation = image_generation;
        s.confirmed = 0;
        s.remaining_tries = allowed;
        self.candidate_slot = slot as u8;
        Ok(())
    }

    /// CandidateBoot: a boot of the active slot began. Consumes one try, and
    /// stops at zero rather than wrapping.
    ///
    /// Returns the tries left *after* this one. Zero does not refuse this
    /// boot; it means the next [`reboot`](Self::reboot) rolls back.
    pub fn begin_boot(&mut self) -> Result<u8, BootError> {
        let active = self.active()?;
        match self.slot(active).state {
            SlotState::Empty => return Err(BootError::SlotEmpty),
            SlotState::Failed => return Err(BootError::SlotFailed),
            _ => {}
        }
        let s = self.slot_mut(active);
        s.remaining_tries = s.remaining_tries.saturating_sub(1);
        Ok(s.remaining_tries)
    }

    /// HealthCheck passed: the active slot is confirmed and becomes the
    /// rollback target.
    ///
    /// Refused if the slot was never booted (B1) or has failed. On success the
    /// slot's tries are restored, `last_confirmed_slot` is the active slot, and
    /// the candidate — whichever slot it named — is cleared.
    pub fn confirm(&mut self) -> Result<(), BootError> {
        let active = self.active()?;
        let info = self.slot(active);
        if info.state == SlotState::Failed {
            return Err(BootError::SlotFailed);
        }
        if !info.has_been_booted() {
            return Err(BootError::NotBooted);
        }
        let s = self.slot_mut(active);
        s.state = SlotState::Confirmed;
        s.confirmed = 1;
        s.remaining_tries = s.tries_allowed;
        self.last_confirmed_slot = active as u8;
        self.candidate_slot = NO_CANDIDATE;
        Ok(())
    }

    /// HealthCheck failed on the active slot.
    ///
    /// * A **candidate** (an unconfirmed slot being tried) is marked unbootable,
    ///   and the next [`reboot`](Self::reboot) rolls back to the last confirmed
    ///   slot — [`HealthOutcome::CandidateFailed`].
    /// * The **last confirmed slot** is *not* marked: it is the only image the
    ///   system can fall back to, and marking it `Failed` while
    ///   [`begin_boot`](Self::begin_boot) refuses failed slots would leave
    ///   nothing bootable. There is nowhere to roll back to, so the block is
    ///   unchanged — [`HealthOutcome::NothingToRollBackTo`] — and the caller
    ///   must not reset on it: a reset that changes nothing, with no persisted
    ///   try counter, is a boot loop.
    ///
    /// This only records the verdict; what follows from it is
    /// [`reboot`](Self::reboot), so a caller that records a failure and then
    /// cannot reset has not lost the record.
    pub fn fail_health(&mut self) -> Result<HealthOutcome, BootError> {
        let active = self.active()?;
        if active == self.last_confirmed()? {
            return Ok(HealthOutcome::NothingToRollBackTo);
        }
        self.slot_mut(active).state = SlotState::Failed;
        Ok(HealthOutcome::CandidateFailed)
    }

    /// A health report arrived: `healthy` says whether the boot met its health
    /// target. The block — not the reporter — decides what that means.
    ///
    /// RFC-0.33-001 E: the service that *observes* health reports it and never
    /// commands a confirm or a rollback; this is where the decision is made.
    pub fn apply_health(&mut self, healthy: bool) -> Result<HealthVerdict, BootError> {
        if healthy {
            self.confirm()?;
            return Ok(HealthVerdict::Confirmed);
        }
        Ok(match self.fail_health()? {
            HealthOutcome::CandidateFailed => HealthVerdict::MustRollBack,
            HealthOutcome::NothingToRollBackTo => HealthVerdict::NoFallback,
        })
    }

    /// Rollback, or the switch to a staged candidate: choose the slot the next
    /// boot runs, and return it.
    ///
    /// * If the active slot failed its health check or is out of tries, roll
    ///   back: `active = last_confirmed`, its tries restored, and **the
    ///   candidate cleared and marked unbootable** (ADR-0009).
    /// * Otherwise, if a healthy candidate is staged, switch to it. Its tries are
    ///   its own: the policy is `tries_allowed` boots *per staged image*.
    /// * Otherwise nothing changes.
    pub fn reboot(&mut self) -> Result<SlotId, BootError> {
        let active = self.active()?;
        let last_confirmed = self.last_confirmed()?;
        let candidate = self.candidate()?;
        let info = *self.slot(active);
        let unhealthy = info.state == SlotState::Failed;
        let out_of_tries = info.remaining_tries == 0;

        if unhealthy || out_of_tries {
            if let Some(c) = candidate {
                self.slot_mut(c).state = SlotState::Failed;
            }
            self.candidate_slot = NO_CANDIDATE;
            self.active_slot = last_confirmed as u8;
            let target = self.slot_mut(last_confirmed);
            target.remaining_tries = target.tries_allowed;
            return Ok(last_confirmed);
        }

        if let Some(c) = candidate {
            if self.slot(c).state != SlotState::Failed && c != active {
                // The candidate keeps the tries it was staged with. They are
                // NOT overwritten with the outgoing slot's count: a slot's
                // "has it been booted" is read off its own tries
                // (`has_been_booted`), so a carried-over count would make a
                // slot that has never run look as if it had — and `confirm`
                // would then accept it (B1). Found by the refinement test.
                self.active_slot = c as u8;
                return Ok(c);
            }
        }
        Ok(active)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> BootControlBlock {
        BootControlBlock::new(1)
    }

    /// A refusal must change nothing: the block compares equal afterwards.
    fn refused<T: core::fmt::Debug>(
        b: &mut BootControlBlock,
        op: impl FnOnce(&mut BootControlBlock) -> Result<T, BootError>,
        expect: BootError,
    ) {
        let before = *b;
        let r = op(b);
        assert_eq!(r.unwrap_err(), expect);
        assert_eq!(*b, before, "a refused transition changed the block");
    }

    #[test]
    fn the_initial_block_is_a_confirmed_slot_a() {
        let b = fresh();
        assert_eq!(b.active(), Ok(SlotId::A));
        assert_eq!(b.last_confirmed(), Ok(SlotId::A));
        assert_eq!(b.candidate(), Ok(None));
        assert!(b.slot(SlotId::A).has_been_booted());
        assert!(!b.slot(SlotId::B).has_been_booted());
    }

    #[test]
    fn a_boot_consumes_a_try_and_stops_at_zero() {
        let mut b = fresh();
        assert_eq!(b.begin_boot(), Ok(2));
        assert_eq!(b.begin_boot(), Ok(1));
        assert_eq!(b.begin_boot(), Ok(0));
        // Saturates: it does not wrap to 255 and grant 255 more boots.
        assert_eq!(b.begin_boot(), Ok(0));
    }

    #[test]
    fn confirm_restores_tries_and_marks_the_slot() {
        let mut b = fresh();
        b.begin_boot().unwrap();
        b.confirm().unwrap();
        let a = b.slot(SlotId::A);
        assert_eq!(a.state, SlotState::Confirmed);
        assert_eq!((a.confirmed, a.remaining_tries), (1, a.tries_allowed));
        assert_eq!(b.last_confirmed(), Ok(SlotId::A));
    }

    /// Model property B1: a boot that did not happen cannot have been healthy.
    #[test]
    fn a_slot_cannot_be_confirmed_before_it_was_booted() {
        let mut b = fresh();
        b.set_candidate(SlotId::B, 2).unwrap();
        b.reboot().unwrap(); // active is now B, never booted
        assert_eq!(b.active(), Ok(SlotId::B));
        refused(&mut b, |b| b.confirm(), BootError::NotBooted);
        b.begin_boot().unwrap();
        assert_eq!(b.confirm(), Ok(()));
    }

    #[test]
    fn a_failed_slot_cannot_be_booted_or_confirmed() {
        let mut b = fresh();
        b.set_candidate(SlotId::B, 2).unwrap();
        b.reboot().unwrap();
        assert_eq!(b.fail_health(), Ok(HealthOutcome::CandidateFailed));
        refused(&mut b, |b| b.begin_boot(), BootError::SlotFailed);
        refused(&mut b, |b| b.confirm(), BootError::SlotFailed);
    }

    /// The decision a health report produces, for each state the block can be in.
    #[test]
    fn a_health_report_is_decided_by_the_block() {
        // Confirmed slot, healthy: confirmed (and still confirmed).
        let mut b = fresh();
        b.begin_boot().unwrap();
        assert_eq!(b.apply_health(true), Ok(HealthVerdict::Confirmed));
        assert_eq!(b.slot(SlotId::A).confirmed, 1);

        // Confirmed slot, unhealthy: there is nowhere to go, and nothing changes.
        let mut b = fresh();
        b.begin_boot().unwrap();
        let before = b;
        assert_eq!(b.apply_health(false), Ok(HealthVerdict::NoFallback));
        assert_eq!(b, before);

        // Candidate running, healthy: it becomes the rollback target.
        let mut b = fresh();
        b.set_candidate(SlotId::B, 2).unwrap();
        b.reboot().unwrap();
        b.begin_boot().unwrap();
        assert_eq!(b.apply_health(true), Ok(HealthVerdict::Confirmed));
        assert_eq!(b.last_confirmed(), Ok(SlotId::B));

        // Candidate running, unhealthy: roll back — and only the caller resets.
        let mut b = fresh();
        b.set_candidate(SlotId::B, 2).unwrap();
        b.reboot().unwrap();
        b.begin_boot().unwrap();
        assert_eq!(b.apply_health(false), Ok(HealthVerdict::MustRollBack));
        assert_eq!(b.active(), Ok(SlotId::B), "the verdict is not the rollback");
        assert_eq!(b.reboot(), Ok(SlotId::A));
        assert_eq!(b.slot(SlotId::B).state, SlotState::Failed);
    }

    /// A candidate that was never booted cannot be reported healthy (B1).
    #[test]
    fn a_healthy_report_for_a_slot_that_never_booted_is_refused() {
        let mut b = fresh();
        b.set_candidate(SlotId::B, 2).unwrap();
        b.reboot().unwrap(); // switched to B, but B has not begun a boot
        let before = b;
        assert_eq!(b.apply_health(true), Err(BootError::NotBooted));
        assert_eq!(b, before);
    }

    /// The hazard `fail_health` exists to avoid: marking the only confirmed slot
    /// `Failed` would leave a system that refuses to boot anything.
    #[test]
    fn the_last_confirmed_slot_is_never_marked_failed() {
        let mut b = fresh();
        let before = b;
        assert_eq!(b.fail_health(), Ok(HealthOutcome::NothingToRollBackTo));
        assert_eq!(b, before, "the fallback slot was touched");
        // ...and it still boots.
        assert!(b.begin_boot().is_ok());
    }

    #[test]
    fn an_empty_slot_cannot_be_booted() {
        let mut b = fresh();
        b.active_slot = SlotId::B as u8;
        refused(&mut b, |b| b.begin_boot(), BootError::SlotEmpty);
    }

    /// ADR-0009: an update goes to the *inactive* slot.
    #[test]
    fn staging_into_the_running_slot_is_refused() {
        let mut b = fresh();
        refused(
            &mut b,
            |b| b.set_candidate(SlotId::A, 2),
            BootError::ActiveSlot,
        );
    }

    #[test]
    fn staging_over_the_only_fallback_is_refused() {
        let mut b = fresh();
        b.set_candidate(SlotId::B, 2).unwrap();
        b.reboot().unwrap(); // running B, unconfirmed; A is the only known-good image
        refused(
            &mut b,
            |b| b.set_candidate(SlotId::A, 3),
            BootError::LastConfirmedSlot,
        );
    }

    #[test]
    fn a_candidate_is_switched_to_at_reboot() {
        let mut b = fresh();
        b.set_candidate(SlotId::B, 2).unwrap();
        assert_eq!(b.candidate(), Ok(Some(SlotId::B)));
        assert_eq!(b.reboot(), Ok(SlotId::B));
        assert_eq!(b.active(), Ok(SlotId::B));
        // Still a candidate until it is confirmed.
        assert_eq!(b.candidate(), Ok(Some(SlotId::B)));
        assert_eq!(b.last_confirmed(), Ok(SlotId::A));
    }

    /// The path this line exists for: a candidate boots, fails its health
    /// check, and the next reboot rolls back and marks it unbootable.
    #[test]
    fn a_failed_candidate_rolls_back_and_is_marked_unbootable() {
        let mut b = fresh();
        b.set_candidate(SlotId::B, 2).unwrap();
        b.reboot().unwrap();
        b.begin_boot().unwrap();
        assert_eq!(b.fail_health(), Ok(HealthOutcome::CandidateFailed));
        assert_eq!(b.reboot(), Ok(SlotId::A));
        assert_eq!(b.active(), Ok(SlotId::A));
        assert_eq!(b.candidate(), Ok(None));
        assert_eq!(b.slot(SlotId::B).state, SlotState::Failed);
        // ...and it is not selected again on the next reboot.
        assert_eq!(b.reboot(), Ok(SlotId::A));
    }

    /// Out of tries is the same rollback, without a health verdict: three
    /// unconfirmed boots in a row and the system falls back.
    #[test]
    fn running_out_of_tries_rolls_back() {
        let mut b = fresh();
        b.set_candidate(SlotId::B, 2).unwrap();
        b.reboot().unwrap();
        for _ in 0..3 {
            b.begin_boot().unwrap();
        }
        assert_eq!(b.slot(SlotId::B).remaining_tries, 0);
        assert_eq!(b.reboot(), Ok(SlotId::A));
        assert_eq!(b.slot(SlotId::B).state, SlotState::Failed);
    }

    /// A rollback restores the fallback's tries, so the system is not one boot
    /// from another rollback the moment it lands.
    #[test]
    fn a_rollback_restores_the_fallbacks_tries() {
        let mut b = fresh();
        b.begin_boot().unwrap(); // A: 2 left
        b.set_candidate(SlotId::B, 2).unwrap();
        b.reboot().unwrap();
        b.fail_health().unwrap();
        b.reboot().unwrap();
        let a = b.slot(SlotId::A);
        assert_eq!(a.remaining_tries, a.tries_allowed);
    }

    /// Staging a new image into a slot that failed before must not inherit the
    /// failure — otherwise the fresh image rolls back without ever booting.
    #[test]
    fn restaging_a_failed_slot_clears_the_failure() {
        let mut b = fresh();
        b.set_candidate(SlotId::B, 2).unwrap();
        b.reboot().unwrap();
        b.fail_health().unwrap();
        b.reboot().unwrap();
        assert_eq!(b.slot(SlotId::B).state, SlotState::Failed);
        b.set_candidate(SlotId::B, 3).unwrap();
        let s = b.slot(SlotId::B);
        assert_eq!(
            (s.state, s.confirmed, s.remaining_tries, s.image_generation),
            (SlotState::Candidate, 0, s.tries_allowed, 3)
        );
    }

    #[test]
    fn a_corrupt_slot_byte_is_an_error_not_a_panic() {
        let mut b = fresh();
        b.active_slot = 7;
        assert_eq!(b.active(), Err(BootError::InvalidSlot(7)));
        refused(&mut b, |b| b.begin_boot(), BootError::InvalidSlot(7));
        refused(&mut b, |b| b.reboot(), BootError::InvalidSlot(7));
        let mut b = fresh();
        b.candidate_slot = 9;
        assert_eq!(b.candidate(), Err(BootError::InvalidSlot(9)));
    }

    #[test]
    fn a_reboot_with_nothing_to_do_changes_nothing() {
        let mut b = fresh();
        let before = b;
        assert_eq!(b.reboot(), Ok(SlotId::A));
        assert_eq!(b, before);
    }
}
