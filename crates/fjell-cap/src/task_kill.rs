//! Which tasks a `TASK_KILL` holder may **not** destroy (RFC-0.49-001 D2.3), as a pure function.
//!
//! The kernel hands the two ids here after the capability check (kind `TaskControl`, right `TASK_KILL`, scope) has
//! already passed, so the rule is testable on the host and each clause can be shown red by mutation. The capability
//! decides *who may kill*; this decides *what may never be killed*:
//!
//! * **the caller itself** -- `sys_exit` is the verb for ending yourself, and a kill that destroys its own caller returns
//!   to nothing;
//! * **the idle task** (slot 0) -- the scheduler's fallback; with it gone there is nothing to run when nothing is runnable.
//!
//! **Not refused, on purpose:** `init` and `service-manager`. A `TASK_KILL` holder is trusted with them; a kill right that
//! cannot be used on the supervisor is a rule that will be broken the first day it is needed.

use fjell_abi::error::SysError;
use fjell_abi::task::TaskId;

/// The idle task's slot index. The kernel builds it first, in slot 0, and nothing may destroy it.
pub const IDLE_INDEX: u16 = 0;

/// `Ok(())` if `caller` may destroy `target`; `Err(PermissionDenied)` for itself and for the idle task.
pub fn kill_target_permitted(target: TaskId, caller: TaskId) -> Result<(), SysError> {
    if target.index == IDLE_INDEX || target.index == caller.index {
        Err(SysError::PermissionDenied)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CALLER: TaskId = TaskId::new(7, 2);

    #[test]
    fn another_ordinary_task_may_be_killed() {
        assert_eq!(kill_target_permitted(TaskId::new(9, 0), CALLER), Ok(()));
        // `init`, and the supervisor, are deliberately not protected
        assert_eq!(kill_target_permitted(TaskId::new(1, 0), CALLER), Ok(()));
    }

    #[test]
    fn the_caller_may_not_kill_itself_whatever_generation_it_names() {
        assert_eq!(
            kill_target_permitted(CALLER, CALLER),
            Err(SysError::PermissionDenied)
        );
        // a handle with the wrong generation for the caller's own slot is still the caller's slot
        assert_eq!(
            kill_target_permitted(TaskId::new(7, 0), CALLER),
            Err(SysError::PermissionDenied)
        );
    }

    #[test]
    fn the_idle_task_may_never_be_killed() {
        assert_eq!(
            kill_target_permitted(TaskId::new(IDLE_INDEX, 0), CALLER),
            Err(SysError::PermissionDenied)
        );
        // not even by a caller that is itself named idle's index
        assert_eq!(
            kill_target_permitted(TaskId::new(IDLE_INDEX, 3), TaskId::new(IDLE_INDEX, 0)),
            Err(SysError::PermissionDenied)
        );
    }

    // ── The funnel half: what `sys_task_kill` asks of the capability *before* it looks at the target ───────────────────
    //
    // Kernel boot cannot show the revoked-lease refusal for `TaskControl` (a revoked `TaskControl` needs one that is not
    // already bound to its holder's boot lease, which would be a third fixture exemption in `bootstrap-leases`). The funnel is
    // one function for every kind, so it is shown here, on the exact capability and rights `sys_task_kill` requires.

    use crate::CSpace;
    use crate::enforcement::require_cap;
    use crate::handle::CapHandle;
    use crate::rights::{CapError, CapKind, CapRights, CapState, ObjectScope};
    use crate::slot::{Capability, LeaseBinding, LeaseChecker};
    use fjell_abi::lease::{LeaseEpoch, LeaseId};

    struct Live;
    impl LeaseChecker for Live {
        fn check_active(&self, _: LeaseId, _: LeaseEpoch) -> Result<(), CapError> {
            Ok(())
        }
    }
    struct Revoked;
    impl LeaseChecker for Revoked {
        fn check_active(&self, _: LeaseId, _: LeaseEpoch) -> Result<(), CapError> {
            Err(CapError::LeaseRevoked)
        }
    }

    fn task_control(rights: CapRights, scope: ObjectScope, bound: bool) -> (CSpace, CapHandle) {
        let mut cs = CSpace::new();
        cs.install_raw(
            0,
            Capability {
                kind: CapKind::TaskControl,
                object_id: 0,
                rights,
                badge: 0,
                scope,
                state: CapState::Active,
                parent: None,
                lease: bound.then_some(LeaseBinding {
                    lease_id: LeaseId::new(0, 1),
                    epoch_at_issue: LeaseEpoch(1),
                }),
            },
        )
        .unwrap();
        (cs, CapHandle::new(0, 0))
    }

    fn ask(
        cs: &CSpace,
        h: CapHandle,
        target: TaskId,
        checker: &dyn LeaseChecker,
    ) -> Result<(), CapError> {
        require_cap(
            cs,
            h,
            CapKind::TaskControl,
            CapRights::TASK_KILL,
            Some(&ObjectScope::Task(target)),
            checker,
        )
        .map(|_| ())
    }

    #[test]
    fn a_task_kill_capability_passes_the_funnel_for_any_target_when_unscoped() {
        let (cs, h) = task_control(CapRights::TASK_KILL, ObjectScope::Any, false);
        assert_eq!(ask(&cs, h, TaskId::new(9, 4), &Live), Ok(()));
    }

    #[test]
    fn without_task_kill_the_capability_is_refused_before_any_target_is_looked_at() {
        let (cs, h) = task_control(
            CapRights(CapRights::ALL_NON_META.0 & !CapRights::TASK_KILL.0),
            ObjectScope::Any,
            false,
        );
        assert!(ask(&cs, h, TaskId::new(9, 0), &Live).is_err());
        // ... and the aggregate that init's grant carries (ALL_NON_META minus TASK_KILL) really lacks it
        assert!(
            !CapRights(CapRights::ALL_NON_META.0 & !CapRights::TASK_KILL.0)
                .contains(CapRights::TASK_KILL)
        );
        assert!(
            CapRights::ALL_NON_META.contains(CapRights::TASK_KILL),
            "the blanket DOES carry it: that is the finding"
        );
    }

    #[test]
    fn a_revoked_lease_refuses_the_kill_with_lease_revoked() {
        let (cs, h) = task_control(CapRights::TASK_KILL, ObjectScope::Any, true);
        assert_eq!(
            ask(&cs, h, TaskId::new(9, 0), &Live),
            Ok(()),
            "control: bound and live passes"
        );
        assert_eq!(
            ask(&cs, h, TaskId::new(9, 0), &Revoked),
            Err(CapError::LeaseRevoked)
        );
    }

    #[test]
    fn a_capability_scoped_to_one_task_does_not_reach_another_or_the_same_slot_later() {
        let (cs, h) = task_control(
            CapRights::TASK_KILL,
            ObjectScope::Task(TaskId::new(9, 0)),
            false,
        );
        assert_eq!(ask(&cs, h, TaskId::new(9, 0), &Live), Ok(()));
        assert!(
            ask(&cs, h, TaskId::new(10, 0), &Live).is_err(),
            "another slot"
        );
        assert!(
            ask(&cs, h, TaskId::new(9, 1), &Live).is_err(),
            "the same slot, next occupant: scope names a generation"
        );
    }
}
