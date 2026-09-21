//! RFC-0.33-001 D1/R2 — the state machine on the block is checked against the
//! model.
//!
//! `fjell-bootctl-model` models health failure and last-known-good fallback,
//! with tests, and until this file **no crate depended on it**: the best
//! specified component in this area described a runtime that did not exist.
//! `fjell_upgrade_format::boot_state` is that runtime, on the type ADR-0009
//! names. This test runs the same operations through both and compares what
//! each says after every step.
//!
//! # The operations are the ones the runtime performs
//!
//! Not every sequence the model's own proptests generate can happen. A boot is
//! a trial of the *active* slot, so the runtime's language is:
//!
//! ```text
//! Stage(inactive slot) | Boot | Confirm | HealthFail | Reboot
//! ```
//!
//! each applied to the active slot, with the guards the real callers have (you
//! do not confirm a slot that failed its health check). The generator produces
//! only those, and a guard that fails means the op is skipped for *both*.
//!
//! # What is compared
//!
//! Everything both sides can say — for each slot whether an image is present,
//! confirmed, has been booted and is healthy; which slot is active, which is
//! the candidate, which is the last confirmed; and how many unconfirmed boots
//! have happened. A difference is a difference in what the next boot does.

use fjell_bootctl_model::{BOOT_COUNT_MAX, BootModel, Slot};
use fjell_upgrade_format::{BootControlBlock, HealthOutcome, SlotId, SlotState};
use proptest::prelude::*;

/// One slot, as either side can describe it.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
struct SlotView {
    installed: bool,
    confirmed: bool,
    booted_once: bool,
    /// Only meaningful for an installed slot: an empty slot has no health.
    healthy: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct View {
    active: Slot,
    pending: Option<Slot>,
    last_known_good: Slot,
    boot_count: u8,
    slots: [SlotView; 2],
}

fn slot_of(id: SlotId) -> Slot {
    match id {
        SlotId::A => Slot::A,
        SlotId::B => Slot::B,
    }
}

fn id_of(s: Slot) -> SlotId {
    match s {
        Slot::A => SlotId::A,
        Slot::B => SlotId::B,
    }
}

fn model_view(m: &BootModel) -> View {
    let sv = |s: Slot| {
        let st = m.slot(s);
        SlotView {
            installed: st.installed,
            confirmed: st.confirmed,
            booted_once: st.booted_once,
            healthy: st.installed && st.health_ok,
        }
    };
    View {
        active: m.active,
        pending: m.pending,
        last_known_good: m.last_known_good,
        boot_count: m.boot_count_since_confirm,
        slots: [sv(Slot::A), sv(Slot::B)],
    }
}

/// The abstraction function: what the block says, in the model's terms.
fn block_view(b: &BootControlBlock) -> View {
    let active = b.active().expect("valid active slot");
    let sv = |id: SlotId| {
        let s = b.slot(id);
        SlotView {
            installed: s.state != SlotState::Empty,
            confirmed: s.confirmed == 1,
            booted_once: s.has_been_booted(),
            healthy: s.state != SlotState::Empty && s.state != SlotState::Failed,
        }
    };
    View {
        active: slot_of(active),
        pending: b.candidate().expect("valid candidate").map(slot_of),
        last_known_good: slot_of(b.last_confirmed().expect("valid lkg")),
        boot_count: b.slot(active).tries_allowed - b.slot(active).remaining_tries,
        slots: [sv(SlotId::A), sv(SlotId::B)],
    }
}

#[derive(Clone, Copy, Debug)]
enum Op {
    Stage(Slot),
    Boot,
    Confirm,
    HealthFail,
    Reboot,
}

/// Apply `op` to both, if the runtime could issue it here. Returns the two
/// results, or `None` if the guard holds it back.
fn step(m: &mut BootModel, b: &mut BootControlBlock, op: Op) -> Option<(bool, bool)> {
    let active = m.active;
    match op {
        Op::Stage(s) => {
            // Only the inactive, non-fallback slot can be staged into.
            if s == active || s == m.last_known_good {
                return None;
            }
            m.set_pending(s);
            let r = b.set_candidate(id_of(s), 2);
            Some((true, r.is_ok()))
        }
        Op::Boot => {
            if !m.slot(active).installed || !m.slot(active).health_ok {
                return None;
            }
            m.mark_booted(active);
            Some((true, b.begin_boot().is_ok()))
        }
        Op::Confirm => {
            if !m.slot(active).installed || !m.slot(active).health_ok {
                return None;
            }
            let mr = m.confirm_slot(active).is_ok();
            Some((mr, b.confirm().is_ok()))
        }
        Op::HealthFail => {
            // Not compared when the failing slot is the last confirmed one.
            // The model marks it unhealthy and then "rolls back" to it — a
            // system with nothing bootable, which the model never says is
            // wrong. The block refuses to mark its only fallback and reports
            // `HealthOutcome::NothingToRollBackTo`, and the runtime does not
            // reset on that (a reset changes nothing and, with no persisted
            // try counter, loops). The block's side of this is asserted in
            // `boot_state`'s own tests; the model's side is a finding.
            if !m.slot(active).installed || active == m.last_known_good {
                return None;
            }
            m.health_fail(active);
            Some((true, b.fail_health() == Ok(HealthOutcome::CandidateFailed)))
        }
        Op::Reboot => {
            m.reboot();
            Some((true, b.reboot().is_ok()))
        }
    }
}

fn arb_slot() -> impl Strategy<Value = Slot> {
    prop_oneof![Just(Slot::A), Just(Slot::B)]
}

fn arb_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        arb_slot().prop_map(Op::Stage),
        Just(Op::Boot),
        Just(Op::Confirm),
        Just(Op::HealthFail),
        Just(Op::Reboot),
    ]
}

/// The two start in the same state.
#[test]
fn both_start_from_the_same_state() {
    let (m, b) = (BootModel::new(), BootControlBlock::new(1));
    let (mv, bv) = (model_view(&m), block_view(&b));
    // An empty slot's health is not compared, so normalise it.
    assert_eq!(mv.active, bv.active);
    assert_eq!(mv.pending, bv.pending);
    assert_eq!(mv.last_known_good, bv.last_known_good);
    assert_eq!(mv.boot_count, bv.boot_count);
    assert_eq!(mv.slots, bv.slots);
    assert_eq!(BOOT_COUNT_MAX, b.slot_a.tries_allowed);
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 4_000, ..ProptestConfig::default() })]

    /// After every step, the block and the model agree about everything both
    /// can say — and where the model accepts an operation, so does the block.
    #[test]
    fn the_block_refines_the_model(ops in prop::collection::vec(arb_op(), 0..=40)) {
        let mut m = BootModel::new();
        let mut b = BootControlBlock::new(1);
        for (i, op) in ops.iter().enumerate() {
            let Some((model_ok, block_ok)) = step(&mut m, &mut b, *op) else { continue };
            prop_assert_eq!(
                model_ok, block_ok,
                "step {} {:?}: the model {} it and the block {} it", i, op,
                if model_ok { "accepts" } else { "refuses" },
                if block_ok { "accepts" } else { "refuses" }
            );
            prop_assert_eq!(model_view(&m), block_view(&b), "after step {} {:?}", i, op);
        }
    }
}
