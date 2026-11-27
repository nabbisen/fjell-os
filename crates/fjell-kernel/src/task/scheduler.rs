#![allow(dead_code)]
//! Fixed-priority round-robin scheduler. The ready queue is `fjell-task::ready_queue`.
//!
//! Invariants (SCHED-*):
//!   SCHED-001  The same TaskId never appears twice in the ready queue.
//!   SCHED-002  The running task is not in the ready queue.
//!   SCHED-003  Faulted/Exited tasks are never enqueued.
//!   SCHED-004  `choose_next` returns only Runnable or idle tasks.
//!   SCHED-005  The idle task never reaches Exited.

use super::id::TaskId;

// The multi-level ready queue, its priorities and its tests live in `fjell-task` (RFC-0.50-001): `ReadyQueue::remove` was written at 0.49
// with no test and hung the machine, found only by booting. `Scheduler` stays here because `set_current` tells the audit ring who has the CPU.
pub use fjell_task::ready_queue::{PRIORITY_IDLE, PRIORITY_USER, ReadyQueue};

// ── SchedError ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedError {
    QueueFull,
    AlreadyEnqueued,
}

// ── Scheduler ────────────────────────────────────────────────────────────────

/// Kernel scheduler state.
pub struct Scheduler {
    ready: ReadyQueue,
    current: Option<TaskId>,
    idle_task_id: Option<TaskId>,
    tick: u64,
}

impl Scheduler {
    pub const fn new() -> Self {
        Scheduler {
            ready: ReadyQueue::new(),
            current: None,
            idle_task_id: None,
            tick: 0,
        }
    }

    pub fn set_idle(&mut self, id: TaskId) {
        self.idle_task_id = Some(id);
    }

    /// Forget `id` entirely: it is not in the ready queue afterwards, and it is not the current task (RFC-0.49-001 D5).
    pub fn forget(&mut self, id: TaskId) {
        self.ready.remove(id);
        if self.current == Some(id) {
            self.current = None;
        }
    }

    /// Enqueue a runnable task.
    pub fn enqueue_runnable(&mut self, id: TaskId, priority: u8) {
        self.ready.enqueue(id, priority);
    }

    /// Choose the next task to run (or idle if nothing is ready).
    pub fn choose_next(&mut self) -> TaskId {
        self.ready
            .dequeue_next()
            .or(self.idle_task_id)
            .expect("idle task must always be set (SCHED-005)")
    }

    /// Handle `sys_yield`: re-enqueue current at its priority, clear current.
    ///
    /// Does NOT choose the next task — `schedule_next` calls `choose_next`.
    pub fn on_yield(&mut self, current: TaskId, priority: u8) {
        self.ready.enqueue(current, priority);
        self.current = None;
        // Note: schedule_next in the trap dispatcher calls choose_next()
        // after this returns; we must NOT call it here too or the next
        // task would be dequeued and discarded.
    }

    /// Suspend the running task (e.g. IPC block): clear `current` without
    /// popping anything from the ready queue.
    ///
    /// The task's state must already have been changed to `Blocked` by the
    /// caller.  `schedule_next` in the trap dispatcher will call `choose_next`
    /// to pick the actual successor.
    pub fn suspend_current(&mut self) {
        self.current = None;
    }

    /// Handle `sys_exit` or task completion: clear current.
    ///
    /// Does NOT choose the next task — `schedule_next` calls `choose_next`.
    pub fn on_exit(&mut self) {
        self.current = None;
    }

    /// Handle a user fault: task is already marked Faulted; clear current.
    ///
    /// Does NOT choose the next task — `schedule_next` calls `choose_next`.
    pub fn on_fault(&mut self) {
        self.current = None;
    }

    /// Timer tick: optionally preempt current task.
    pub fn tick(&mut self, current: TaskId, priority: u8) -> Option<TaskId> {
        self.tick += 1;
        // Simple policy: preempt every tick to give other tasks a chance.
        // In M2 with only a few tasks this is fine.
        self.ready.enqueue(current, priority);
        self.current = None;
        Some(self.choose_next())
    }

    pub fn set_current(&mut self, id: TaskId) {
        self.current = Some(id);
        // RFC-0.41-006 R1: say who has the CPU to the audit ring, which cannot ask.
        crate::audit::ring::AUDIT.note_running(id.index);
    }

    pub fn current(&self) -> Option<TaskId> {
        self.current
    }

    pub fn ticks(&self) -> u64 {
        self.tick
    }
}
