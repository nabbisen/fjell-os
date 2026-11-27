//! The multi-level ready queue (moved from `crates/fjell-kernel/src/task/scheduler.rs`, RFC-0.50-001).
//!
//! Invariants (SCHED-*) -- the kernel's `Scheduler` upholds them on top of this:
//!   SCHED-001  The same TaskId never appears twice in the ready queue.
//!   SCHED-002  The running task is not in the ready queue.
//!   SCHED-003  Faulted/Exited tasks are never enqueued.
//!   SCHED-004  `choose_next` returns only Runnable or idle tasks.
//!   SCHED-005  The idle task never reaches Exited.

use crate::tcb::MAX_TASKS;
use fjell_abi::task::TaskId;

// ── Priority constants ────────────────────────────────────────────────────────

pub const PRIORITY_IDLE: u8 = 0;
pub const PRIORITY_USER: u8 = 32;
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
    head: usize,
    len: usize,
}

impl TaskQueue {
    const fn new() -> Self {
        TaskQueue {
            items: [None; MAX_TASKS],
            head: 0,
            len: 0,
        }
    }

    fn push(&mut self, id: TaskId) -> bool {
        if self.len == MAX_TASKS {
            return false;
        }
        let tail = (self.head + self.len) % MAX_TASKS;
        self.items[tail] = Some(id);
        self.len += 1;
        true
    }

    fn pop(&mut self) -> Option<TaskId> {
        if self.len == 0 {
            return None;
        }
        let id = self.items[self.head].take();
        self.head = (self.head + 1) % MAX_TASKS;
        self.len -= 1;
        id
    }

    fn contains(&self, id: TaskId) -> bool {
        for i in 0..self.len {
            let idx = (self.head + i) % MAX_TASKS;
            if self.items[idx] == Some(id) {
                return true;
            }
        }
        false
    }

    /// Remove every occurrence of `id`, keeping the order of the rest. Returns how many were removed.
    fn remove(&mut self, id: TaskId) -> usize {
        let mut kept = TaskQueue::new();
        let mut removed = 0;
        while let Some(v) = self.pop() {
            if v == id {
                removed += 1;
            } else {
                kept.push(v);
            }
        }
        *self = kept;
        removed
    }
}

// ── ReadyQueue ────────────────────────────────────────────────────────────────

/// Multi-level ready queue.
pub struct ReadyQueue {
    queues: [TaskQueue; MAX_PRIORITY_LEVELS],
    non_empty_mask: u8, // bitmask of non-empty buckets
}

impl ReadyQueue {
    pub const fn new() -> Self {
        const EMPTY_Q: TaskQueue = TaskQueue::new();
        ReadyQueue {
            queues: [EMPTY_Q; MAX_PRIORITY_LEVELS],
            non_empty_mask: 0,
        }
    }

    /// Take `id` out of every bucket (RFC-0.49-001 D5). A destroyed `Runnable` task is in the ready queue; leaving it there
    /// would hand the CPU to a slot whose occupant is gone or, after a reuse, to a different task. Returns how many entries
    /// were removed (0 or 1 by SCHED-001).
    pub fn remove(&mut self, id: TaskId) -> usize {
        let mut removed = 0;
        for (bucket, q) in self.queues.iter_mut().enumerate() {
            removed += q.remove(id);
            // `dequeue_next` finds its bucket through this mask, so a bucket this empties must leave it -- otherwise the
            // next dequeue pops nothing from it and hands the CPU to idle while a lower bucket still has runnable tasks.
            if q.len == 0 {
                self.non_empty_mask &= !(1 << bucket);
            }
        }
        removed
    }

    pub fn enqueue(&mut self, id: TaskId, priority: u8) -> bool {
        let bucket = priority_to_bucket(priority);
        if self.queues[bucket].contains(id) {
            return false;
        } // SCHED-001
        let ok = self.queues[bucket].push(id);
        if ok {
            self.non_empty_mask |= 1 << bucket;
        }
        ok
    }

    /// Dequeue the highest-priority waiting task.
    pub fn dequeue_next(&mut self) -> Option<TaskId> {
        // Find the highest non-empty bucket (MSB of mask).
        if self.non_empty_mask == 0 {
            return None;
        }
        let bucket = (7 - self.non_empty_mask.leading_zeros()) as usize;
        let id = self.queues[bucket].pop();
        if self.queues[bucket].len == 0 {
            self.non_empty_mask &= !(1 << bucket);
        }
        id
    }
}

// ── host-side unit tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn id(i: u16) -> TaskId {
        TaskId::new(i, 0)
    }

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

    // ── `remove` (RFC-0.50-001 D7) ───────────────────────────────────────────────
    //
    // `ReadyQueue::remove` was written at 0.49 for `TaskKill` and **had no test**. It left a `non_empty_mask` bit set on a bucket
    // it had emptied, and the machine hung: `dequeue_next` finds its bucket through the mask, popped nothing from the emptied
    // one, and handed the CPU to idle while a lower bucket still had runnable tasks. It was found by booting QEMU. These are the
    // tests whose absence let that happen; each was shown red against the pre-fix code (the mask-clearing block deleted).

    fn mask(rq: &ReadyQueue) -> u8 {
        rq.non_empty_mask
    }

    #[test]
    fn remove_clears_the_mask_bit_of_a_bucket_it_empties() {
        let mut rq = ReadyQueue::new();
        rq.enqueue(id(0), PRIORITY_KERNEL_SERVICE);
        assert_ne!(mask(&rq), 0);
        assert_eq!(rq.remove(id(0)), 1);
        assert_eq!(
            mask(&rq),
            0,
            "an emptied bucket must leave the mask, or dequeue_next looks in it for ever"
        );
        assert!(rq.dequeue_next().is_none());
    }

    /// The hang itself: remove the only task of the *higher* bucket; the lower bucket's task must still be dequeued.
    #[test]
    fn removing_the_only_task_of_the_higher_bucket_leaves_the_lower_one_runnable() {
        let mut rq = ReadyQueue::new();
        rq.enqueue(id(1), PRIORITY_USER);
        rq.enqueue(id(2), PRIORITY_KERNEL_SERVICE);
        assert_eq!(rq.remove(id(2)), 1);
        assert_eq!(
            rq.dequeue_next(),
            Some(id(1)),
            "handing the CPU to idle here is the 0.49 hang"
        );
        assert_eq!(mask(&rq), 0);
    }

    #[test]
    fn remove_of_a_task_that_is_not_queued_removes_nothing_and_disturbs_nothing() {
        let mut rq = ReadyQueue::new();
        rq.enqueue(id(1), PRIORITY_USER);
        let before = mask(&rq);
        assert_eq!(rq.remove(id(9)), 0);
        assert_eq!(mask(&rq), before);
        assert_eq!(rq.dequeue_next(), Some(id(1)));
    }

    #[test]
    fn remove_from_a_bucket_that_keeps_others_keeps_its_bit_and_its_order() {
        let mut rq = ReadyQueue::new();
        rq.enqueue(id(1), PRIORITY_USER);
        rq.enqueue(id(2), PRIORITY_USER);
        rq.enqueue(id(3), PRIORITY_USER);
        let before = mask(&rq);
        assert_eq!(rq.remove(id(2)), 1);
        assert_eq!(mask(&rq), before, "the bucket is not empty: its bit stays");
        assert_eq!(rq.dequeue_next(), Some(id(1)));
        assert_eq!(
            rq.dequeue_next(),
            Some(id(3)),
            "the order of the rest is kept"
        );
    }
}
