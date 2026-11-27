//! The milestone-marker queries over the task table (moved from `crates/fjell-kernel/src/trap/dispatch.rs`, RFC-0.50-001).
//!
//! **The scan only.** In the kernel, `exited_ok_by_image` is `exit_latched(image) || scan` -- the latch is a process-global
//! static set on the exit path (RFC-0.49-001 D1), and it stays there: in a library that parallel host tests share it would carry
//! one test's `Exited(0)` into another's "must not fire while running". These functions are the old definition, which the 0.49
//! latch was added *beside* and not instead of, and which the eight tests below assert unchanged.

use crate::tcb::{TaskState, TaskTable};
use fjell_abi::service::ImageId;

/// Does any task carrying `image_id` sit in `Exited(0)`?
///
/// RFC-v0.23-002: milestone markers must attest the *named service*, not
/// whichever task happens to occupy a table index — task-table indices
/// shift as boot timing changes (confirmed live: RFC-v0.23-001's added IPC
/// latency alone moved a task from index 28 to index 20), so an
/// index-keyed check silently starts attesting a different task.
///
/// Semantics are deliberately simple ("some task with this `ImageId` is
/// `Exited(0)`"), per the handoff's settled design decision: if more than
/// one live task ever carries the same `ImageId` (e.g. a respawn), that is
/// a finding to report, not something for this function to disambiguate.
pub fn exited_ok_by_image(table: &TaskTable, image_id: ImageId) -> bool {
    table
        .iter()
        .any(|t| t.image_id == image_id && matches!(t.state, TaskState::Exited(0)))
}

/// Is every task carrying `image_id` no longer runnable (exited or
/// faulted)? `Iterator::all` is vacuously true when no task carries
/// `image_id` at all, mirroring the old index-keyed `done()`'s
/// `unwrap_or(true)` — "nothing there" counts as done, same as before.
pub fn done_by_image(table: &TaskTable, image_id: ImageId) -> bool {
    table
        .iter()
        .filter(|t| t.image_id == image_id)
        .all(|t| matches!(t.state, TaskState::Exited(_) | TaskState::Faulted(_)))
}

#[cfg(test)]
mod milestone_marker_tests {
    use super::*;
    use crate::AddressSpaceId;
    use crate::tcb::{Task, TaskTable};
    use fjell_abi::task::TaskId;

    /// A minimal, host-constructible task: only `image_id` and `state`
    /// matter for these checks, so the rest are throwaway values.
    fn task_with(image_id: ImageId, state: TaskState) -> Task {
        let mut t = Task::new(TaskId::new(0, 0), 0, AddressSpaceId(0), 0, 0);
        t.image_id = image_id;
        t.state = state;
        t
    }

    // ── Requirement 1: fails when the named service fails ──────────────────

    #[test]
    fn exited_ok_true_when_named_service_exited_cleanly() {
        let mut table = TaskTable::new();
        table
            .insert(task_with(ImageId::SYNCD, TaskState::Exited(0)))
            .unwrap();
        assert!(exited_ok_by_image(&table, ImageId::SYNCD));
    }

    #[test]
    fn exited_ok_false_when_named_service_still_running() {
        let mut table = TaskTable::new();
        table
            .insert(task_with(ImageId::SYNCD, TaskState::Runnable))
            .unwrap();
        assert!(
            !exited_ok_by_image(&table, ImageId::SYNCD),
            "marker must not fire while the named service is still running"
        );
    }

    #[test]
    fn exited_ok_false_when_named_service_exited_with_nonzero_code() {
        let mut table = TaskTable::new();
        table
            .insert(task_with(ImageId::SYNCD, TaskState::Exited(1)))
            .unwrap();
        assert!(
            !exited_ok_by_image(&table, ImageId::SYNCD),
            "marker must not fire when the named service failed (nonzero exit)"
        );
    }

    #[test]
    fn exited_ok_false_when_named_service_faulted() {
        use crate::tcb::{FaultCause, FaultInfo};
        let mut table = TaskTable::new();
        table
            .insert(task_with(
                ImageId::SYNCD,
                TaskState::Faulted(FaultInfo {
                    cause: FaultCause::LoadPageFault,
                    sepc: 0,
                    stval: 0,
                }),
            ))
            .unwrap();
        assert!(
            !exited_ok_by_image(&table, ImageId::SYNCD),
            "marker must not fire when the named service faulted"
        );
    }

    // ── Requirement 2: does not fire when a different task occupies the ────
    // ── index the named service used to use ────────────────────────────────

    /// Reproduces the exact real-world scenario this RFC exists to fix:
    /// insert filler tasks to push a different-identity task into table
    /// index 19 — the hardcoded index `TEST:V0.7-SYNC:PASS` used to key
    /// on — and confirm the identity-based check does not fire for
    /// `SYNCD`, even though *something* at that historical index is
    /// `Exited(0)`.
    #[test]
    fn exited_ok_ignores_a_different_task_at_the_old_hardcoded_index() {
        let mut table = TaskTable::new();
        // Fill indices 0..19 with unrelated, still-running tasks.
        for _ in 0..19 {
            table
                .insert(task_with(ImageId::DEVMGR, TaskState::Runnable))
                .unwrap();
        }
        // Index 19 (the old hardcoded syncd slot) is now occupied by an
        // unrelated task that HAS exited cleanly.
        let imposter_id = table
            .insert(task_with(ImageId::NETD, TaskState::Exited(0)))
            .unwrap();
        assert_eq!(
            imposter_id.index, 19,
            "test setup must actually land the imposter at index 19"
        );

        // The old index-keyed check would have said yes here. The
        // identity-keyed check must not: no task in this table carries
        // ImageId::SYNCD at all.
        assert!(
            !exited_ok_by_image(&table, ImageId::SYNCD),
            "must not fire for SYNCD just because index 19 exited cleanly"
        );
        // Sanity: the imposter's own identity is unaffected.
        assert!(exited_ok_by_image(&table, ImageId::NETD));
    }

    // ── done_by_image (used for the init/M7-FAIL path) ──────────────────────

    #[test]
    fn done_by_image_true_when_no_such_task_exists() {
        let table = TaskTable::new();
        assert!(
            done_by_image(&table, ImageId::INIT),
            "vacuous truth: nothing to be un-done about a task that isn't there"
        );
    }

    #[test]
    fn done_by_image_false_while_task_is_running() {
        let mut table = TaskTable::new();
        table
            .insert(task_with(ImageId::INIT, TaskState::Runnable))
            .unwrap();
        assert!(!done_by_image(&table, ImageId::INIT));
    }

    #[test]
    fn done_by_image_true_after_exit_or_fault() {
        let mut table = TaskTable::new();
        table
            .insert(task_with(ImageId::INIT, TaskState::Exited(1)))
            .unwrap();
        assert!(done_by_image(&table, ImageId::INIT));
    }
}
