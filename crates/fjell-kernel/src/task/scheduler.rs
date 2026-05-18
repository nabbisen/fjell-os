#![allow(dead_code)]
//! Fixed-priority round-robin scheduler.
//!
//! Invariants (SCHED-*):
//!   SCHED-001  The same TaskId never appears twice in the ready queue.
//!   SCHED-002  The running task is not in the ready queue.
//!   SCHED-003  Faulted/Exited tasks are never enqueued.
//!   SCHED-004  `choose_next` returns only Runnable or idle tasks.
//!   SCHED-005  The idle task never reaches Exited.

use super::id::TaskId;
use crate::task::tcb::MAX_TASKS;

// ── Priority constants ────────────────────────────────────────────────────────

pub const PRIORITY_IDLE:           u8 = 0;
pub const PRIORITY_USER:           u8 = 32;
pub const PRIORITY_KERNEL_SERVICE: u8 = 64;

// ── Internal bucket mapping ───────────────────────────────────────────────────

const MAX_PRIORITY_LEVELS: usize = 8;

fn priority_to_bucket(p: u8) -> usize {
    // Map 0..255 into 8 buckets; higher priority → higher bucket index.
    (p as usize) * MAX_PRIORITY_LEVELS / 256
}

// ── TaskQueue (circular buffer per bucket) ───────────────────────────────────

struct TaskQueue {
    items: [Option<TaskId>; MAX_TASKS],
    head:  usize,
    len:   usize,
}

impl TaskQueue {
    const fn new() -> Self {
        TaskQueue {
            items: [None; MAX_TASKS],
            head:  0,
            len:   0,
        }
    }

    fn push(&mut self, id: TaskId) -> bool {
        if self.len == MAX_TASKS { return false; }
        let tail = (self.head + self.len) % MAX_TASKS;
        self.items[tail] = Some(id);
        self.len += 1;
        true
    }

    fn pop(&mut self) -> Option<TaskId> {
        if self.len == 0 { return None; }
        let id = self.items[self.head].take();
        self.head = (self.head + 1) % MAX_TASKS;
        self.len -= 1;
        id
    }

    fn contains(&self, id: TaskId) -> bool {
        for i in 0..self.len {
            let idx = (self.head + i) % MAX_TASKS;
            if self.items[idx] == Some(id) { return true; }
        }
        false
    }
}

// ── ReadyQueue ────────────────────────────────────────────────────────────────

/// Multi-level ready queue.
pub struct ReadyQueue {
    queues:          [TaskQueue; MAX_PRIORITY_LEVELS],
    non_empty_mask:  u8,   // bitmask of non-empty buckets
}

impl ReadyQueue {
    pub const fn new() -> Self {
        const EMPTY_Q: TaskQueue = TaskQueue::new();
        ReadyQueue {
            queues:         [EMPTY_Q; MAX_PRIORITY_LEVELS],
            non_empty_mask: 0,
        }
    }

    pub fn enqueue(&mut self, id: TaskId, priority: u8) -> bool {
        let bucket = priority_to_bucket(priority);
        if self.queues[bucket].contains(id) { return false; } // SCHED-001
        let ok = self.queues[bucket].push(id);
        if ok { self.non_empty_mask |= 1 << bucket; }
        ok
    }

    /// Dequeue the highest-priority waiting task.
    pub fn dequeue_next(&mut self) -> Option<TaskId> {
        // Find the highest non-empty bucket (MSB of mask).
        if self.non_empty_mask == 0 { return None; }
        let bucket = (7 - self.non_empty_mask.leading_zeros()) as usize;
        let id = self.queues[bucket].pop();
        if self.queues[bucket].len == 0 {
            self.non_empty_mask &= !(1 << bucket);
        }
        id
    }
}

// ── SchedError ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedError {
    QueueFull,
    AlreadyEnqueued,
}

// ── Scheduler ────────────────────────────────────────────────────────────────

/// Kernel scheduler state.
pub struct Scheduler {
    ready:        ReadyQueue,
    current:      Option<TaskId>,
    idle_task_id: Option<TaskId>,
    tick:         u64,
}

impl Scheduler {
    pub const fn new() -> Self {
        Scheduler {
            ready:        ReadyQueue::new(),
            current:      None,
            idle_task_id: None,
            tick:         0,
        }
    }

    pub fn set_idle(&mut self, id: TaskId) {
        self.idle_task_id = Some(id);
    }

    /// Enqueue a runnable task.
    pub fn enqueue_runnable(&mut self, id: TaskId, priority: u8) {
        self.ready.enqueue(id, priority);
    }

    /// Choose the next task to run (or idle if nothing is ready).
    pub fn choose_next(&mut self) -> TaskId {
        self.ready.dequeue_next()
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
    }

    pub fn current(&self) -> Option<TaskId> {
        self.current
    }

    pub fn ticks(&self) -> u64 {
        self.tick
    }
}

// ── host-side unit tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn id(i: u16) -> TaskId { TaskId::new(i, 0) }

    #[test]
    fn basic_enqueue_dequeue() {
        let mut rq = ReadyQueue::new();
        rq.enqueue(id(0), PRIORITY_USER);
        rq.enqueue(id(1), PRIORITY_USER);
        let a = rq.dequeue_next().unwrap();
        let b = rq.dequeue_next().unwrap();
        assert_ne!(a, b);
        assert!(rq.dequeue_next().is_none());
    }

    #[test]
    fn higher_priority_first() {
        let mut rq = ReadyQueue::new();
        rq.enqueue(id(0), PRIORITY_USER);
        rq.enqueue(id(1), PRIORITY_KERNEL_SERVICE);
        let first = rq.dequeue_next().unwrap();
        assert_eq!(first, id(1), "kernel-service priority should come first");
    }

    #[test]
    fn no_duplicate_enqueue() {
        let mut rq = ReadyQueue::new();
        assert!(rq.enqueue(id(0), PRIORITY_USER));
        assert!(!rq.enqueue(id(0), PRIORITY_USER)); // duplicate → rejected
    }
}
