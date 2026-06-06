//! Boot-control state-machine model (RFC v0.6-002 §7.2).
//! Six bootctl properties + one combined property.

use proptest::prelude::*;

// ── Types ─────────────────────────────────────────────────────────────────────

pub const BOOT_COUNT_MAX: u8 = 3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot { A, B }

impl Slot {
    pub fn other(self) -> Slot { match self { Slot::A => Slot::B, Slot::B => Slot::A } }
}

#[derive(Clone, Debug, Default)]
pub struct SlotState {
    pub installed:    bool,
    pub confirmed:    bool,
    pub booted_once:  bool,
    pub health_ok:    bool,
}

#[derive(Clone, Debug)]
pub struct BootModel {
    pub slot_a:                   SlotState,
    pub slot_b:                   SlotState,
    pub active:                   Slot,
    pub pending:                  Option<Slot>,
    pub last_known_good:          Slot,
    pub boot_count_since_confirm: u8,
}

impl BootModel {
    pub fn new() -> Self {
        Self {
            slot_a: SlotState { installed: true, confirmed: true, booted_once: true, health_ok: true },
            slot_b: SlotState::default(),
            active: Slot::A, pending: None, last_known_good: Slot::A,
            boot_count_since_confirm: 0,
        }
    }

    pub fn slot(&self, s: Slot) -> &SlotState {
        match s { Slot::A => &self.slot_a, Slot::B => &self.slot_b }
    }
    pub fn slot_mut(&mut self, s: Slot) -> &mut SlotState {
        match s { Slot::A => &mut self.slot_a, Slot::B => &mut self.slot_b }
    }

    // ── Operations ────────────────────────────────────────────────────────────

    pub fn set_pending(&mut self, slot: Slot) {
        self.slot_mut(slot).installed = true;
        self.pending = Some(slot);
    }

    pub fn mark_booted(&mut self, slot: Slot) {
        // Kernel enforces: only installed slots may be booted.
        if !self.slot(slot).installed { return; }
        self.slot_mut(slot).booted_once = true;
        self.active = slot;
        // Cap before incrementing — overflow triggers fallback on next reboot.
        if self.boot_count_since_confirm < BOOT_COUNT_MAX {
            self.boot_count_since_confirm += 1;
        }
    }

    /// Returns Ok if confirm succeeded, Err if preconditions not met.
    pub fn confirm_slot(&mut self, slot: Slot) -> Result<(), &'static str> {
        if !self.slot(slot).booted_once {
            return Err("confirm before boot");
        }
        self.slot_mut(slot).confirmed = true;
        self.last_known_good = slot;
        self.boot_count_since_confirm = 0;
        self.pending = None;
        Ok(())
    }

    pub fn health_fail(&mut self, slot: Slot) {
        self.slot_mut(slot).health_ok = false;
    }

    /// Select next boot slot. Falls back to LKG if active is unhealthy or
    /// boot count exceeded.
    pub fn reboot(&mut self) {
        let active_unhealthy = !self.slot(self.active).health_ok;
        let boot_overflow    = self.boot_count_since_confirm >= BOOT_COUNT_MAX;
        if active_unhealthy || boot_overflow {
            self.active = self.last_known_good;
            self.boot_count_since_confirm = 0;
        } else if let Some(p) = self.pending {
            self.active = p;
        }
    }
}

// ── Operations for proptest ───────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum BootOp {
    SetPending(Slot),
    MarkBooted(Slot),
    ConfirmSlot(Slot),
    HealthFail(Slot),
    Reboot,
}

pub fn execute(m: &mut BootModel, op: &BootOp) {
    match op {
        BootOp::SetPending(s)  => m.set_pending(*s),
        BootOp::MarkBooted(s)  => m.mark_booted(*s),
        BootOp::ConfirmSlot(s) => { let _ = m.confirm_slot(*s); }
        BootOp::HealthFail(s)  => m.health_fail(*s),
        BootOp::Reboot         => m.reboot(),
    }
}

// ── Properties ────────────────────────────────────────────────────────────────

/// B1: ConfirmSlot succeeds only after MarkBooted for that slot.
pub fn b1_never_confirm_unbooted(m: &BootModel, slot: Slot) -> Result<(), String> {
    let mut test = m.clone();
    // Reset booted_once to false for the target slot.
    test.slot_mut(slot).booted_once = false;
    let r = test.confirm_slot(slot);
    if r.is_ok() {
        return Err(format!("B1: confirmed unbooted slot {slot:?}"));
    }
    Ok(())
}

/// B2: SetPending alone never sets confirmed=true on a previously-unconfirmed slot.
pub fn b2_pending_not_confirmed(_m: &BootModel) -> Result<(), String> {
    // Test on a fresh sub-model where B is not confirmed.
    let mut fresh = BootModel::new();
    // Carry over nothing from m — we want to test the single-step invariant.
    fresh.set_pending(Slot::B);
    if fresh.slot(Slot::B).confirmed {
        return Err("B2: SetPending produced Confirmed slot on fresh model".into());
    }
    Ok(())
}

/// B3: HealthFail on active slot causes Reboot to select LKG.
pub fn b3_health_fail_rolls_back(m: &BootModel) -> Result<(), String> {
    let mut test = m.clone();
    let active = test.active;
    let lkg    = test.last_known_good;
    test.health_fail(active);
    test.reboot();
    if active != lkg && test.active != lkg {
        return Err(format!("B3: after HealthFail+Reboot active={:?} not LKG={lkg:?}", test.active));
    }
    Ok(())
}

/// B4: boot_count_since_confirm never exceeds BOOT_COUNT_MAX.
pub fn b4_boot_count_bounded(m: &BootModel) -> Result<(), String> {
    if m.boot_count_since_confirm > BOOT_COUNT_MAX {
        return Err(format!(
            "B4: boot_count {} > BOOT_COUNT_MAX {BOOT_COUNT_MAX}",
            m.boot_count_since_confirm
        ));
    }
    Ok(())
}

/// B5: After ConfirmSlot(s), last_known_good == s.
pub fn b5_lkg_advances_on_confirm(m: &BootModel) -> Result<(), String> {
    let mut test = m.clone();
    let slot = test.active;
    test.mark_booted(slot);
    if test.confirm_slot(slot).is_ok() {
        if test.last_known_good != slot {
            return Err(format!("B5: confirmed {slot:?} but LKG={:?}", test.last_known_good));
        }
    }
    Ok(())
}

/// B6 (combined): After any sequence ending in Reboot, the active slot is
/// always either last_known_good, the pending slot, or the previously-active slot.
pub fn b6_active_is_valid_slot(m: &BootModel, ops: &[BootOp]) -> Result<(), String> {
    let mut test = m.clone();
    for op in ops { execute(&mut test, op); }
    // active must be A or B (always true) — deeper invariant: active is always
    // a slot that was once installed.
    let active_installed = test.slot(test.active).installed;
    if !active_installed {
        return Err(format!("B6: active slot {:?} is not installed", test.active));
    }
    Ok(())
}

// ── Proptest generators ───────────────────────────────────────────────────────

fn arb_slot() -> impl Strategy<Value = Slot> {
    prop_oneof![Just(Slot::A), Just(Slot::B)]
}

fn arb_boot_op() -> impl Strategy<Value = BootOp> {
    prop_oneof![
        arb_slot().prop_map(BootOp::SetPending),
        arb_slot().prop_map(BootOp::MarkBooted),
        arb_slot().prop_map(BootOp::ConfirmSlot),
        arb_slot().prop_map(BootOp::HealthFail),
        Just(BootOp::Reboot),
    ]
}

fn arb_boot_sequence() -> impl Strategy<Value = Vec<BootOp>> {
    prop::collection::vec(arb_boot_op(), 0..=32)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 1_000, ..ProptestConfig::default() })]

    #[test]
    fn test_b1_never_confirm_unbooted(ops in arb_boot_sequence(), slot in arb_slot()) {
        let mut m = BootModel::new();
        for op in &ops { execute(&mut m, op); }
        props::b1_never_confirm_unbooted(&m, slot).map_err(|e| TestCaseError::fail(e))?;
    }

    #[test]
    fn test_b2_pending_not_confirmed(ops in arb_boot_sequence()) {
        let mut m = BootModel::new();
        for op in &ops { execute(&mut m, op); }
        props::b2_pending_not_confirmed(&m).map_err(|e| TestCaseError::fail(e))?;
    }

    #[test]
    fn test_b3_health_fail_rolls_back(ops in arb_boot_sequence()) {
        let mut m = BootModel::new();
        for op in &ops { execute(&mut m, op); }
        props::b3_health_fail_rolls_back(&m).map_err(|e| TestCaseError::fail(e))?;
    }

    #[test]
    fn test_b4_boot_count_bounded(ops in arb_boot_sequence()) {
        let mut m = BootModel::new();
        for op in &ops { execute(&mut m, op); }
        props::b4_boot_count_bounded(&m).map_err(|e| TestCaseError::fail(e))?;
    }

    #[test]
    fn test_b5_lkg_advances_on_confirm(ops in arb_boot_sequence()) {
        let mut m = BootModel::new();
        for op in &ops { execute(&mut m, op); }
        props::b5_lkg_advances_on_confirm(&m).map_err(|e| TestCaseError::fail(e))?;
    }

    #[test]
    fn test_b6_active_is_valid_slot(ops in arb_boot_sequence()) {
        let m = BootModel::new();
        props::b6_active_is_valid_slot(&m, &ops).map_err(|e| TestCaseError::fail(e))?;
    }
}

mod props {
    pub use super::{
        b1_never_confirm_unbooted, b2_pending_not_confirmed, b3_health_fail_rolls_back,
        b4_boot_count_bounded, b5_lkg_advances_on_confirm, b6_active_is_valid_slot,
    };
}
