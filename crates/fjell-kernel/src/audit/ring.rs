//! Fixed-capacity, append-only audit ring.
//!
//! # Drain cursor
//! `sys_audit_drain` uses `peek_at` + `advance` (RFC 053) — no drain cursor needed.
//! space.  After draining, [`AuditRing::compact`] is called to reclaim slots
//! from the front of the ring so the ring never gets permanently stuck when
//! full.

use core::cell::Cell;

use fjell_abi::task::TaskId;

/// Maximum number of events stored in the kernel ring. Defined once, in
/// `fjell-audit-format`, because `auditd` needs it to say where a loss happened.
pub use fjell_audit_format::AUDIT_RING_CAPACITY;

/// RFC-0.41-001 R7: whether `TaskSwitch` records are appended at all. **The one
/// place this policy lives** -- change this line to reverse the default.
///
/// Off, because the ring drops the **newest** record when full: a kind that can
/// arrive unboundedly (about 32 per drain while services start) evicts the bounded
/// kinds that matter -- a flood of scheduling telemetry destroys the revocation
/// that follows it. Scheduling events are telemetry; the trail is for events an
/// operator must be able to account for afterwards. The filter is applied where
/// the record is *appended*, not where it is rendered: a record dropped at the
/// console has already consumed a ring slot.
///
/// This is a design position and the owner may reverse it. The correctness half
/// does not depend on it: if it is turned on, `auditd` reports the loss.
pub const RECORD_TASK_SWITCH: bool = false;

/// Internal audit event kind (kernel-private; mapped to `fjell-audit-format`
/// types before export to user space).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
#[repr(u16)]
pub enum AuditKindInternal {
    Boot = 0,
    VmMap = 1,
    VmFault = 2,
    TaskCreate = 3,
    TaskSwitch = 4,
    TaskExit = 5,
    TaskFault = 6,
    Syscall = 7,
    UnknownSyscall = 8,
    // M3 capability events
    CapCopy = 10,
    CapMint = 11,
    CapDelete = 12,
    CapRevoke = 13,
    /// RFC 032 (v0.2.0): explicit slot drop via sys_cap_drop.
    CapDrop = 15,
    // M3 IPC events
    IpcSend = 20,
    IpcRecv = 21,
    IpcCall = 22,
    IpcReply = 23,
    IpcDenied = 24,
    /// RFC 037 (v0.2.0): task consumed its quantum without voluntary yield.
    TaskQuantumExceeded = 30,
    // Lease events (RFC-v0.7.4-003 / W-H-02)
    /// RFC-0.40-002 (closes E-092): **16**, the value the published
    /// `fjell_audit_format::AuditKind::LeaseRevoked` has. It was 40, a number the
    /// published decoder did not know, so every lease revocation decoded as
    /// `Internal` and the published variant was never emitted by anything. The
    /// published enum is the ABI and this is the implementation, so this moved.
    LeaseRevoked = 16,
    // `CapDenied = 41` was here until RFC-0.40-002: emitted by nothing and absent
    // from the published decoder, so a kind nothing emits and nothing decodes. It
    // is gone rather than left to suggest that capability denials are audited
    // (they are not). Auditing them would be new behaviour and its own decision.
}

/// A single audit record stored in the ring.
#[derive(Clone, Copy)]
pub struct AuditRecord {
    pub seq: u64,
    pub tick: u64,
    pub kind: AuditKindInternal,
    pub arg0: usize,
    pub arg1: usize,
    pub result: isize,
}

/// The global audit ring.
pub static AUDIT: AuditRing = AuditRing::new();

/// Append-only fixed-capacity audit ring with drain cursor.
///
/// # Thread-safety
/// Single-hart: `Cell` is used for interior mutability without locking.
/// In SMP this must be replaced with a spinlock or lock-free structure.
pub struct AuditRing {
    records: [Cell<Option<AuditRecord>>; AUDIT_RING_CAPACITY],
    /// Index of the OLDEST record in `records` (modular).
    head: Cell<usize>,
    /// Number of records currently in the ring.
    len: Cell<usize>,
    /// Next sequence number to assign.
    seq: Cell<u64>,
    /// Total records dropped (lifetime counter; never decremented).
    dropped_count: Cell<u64>,
    /// Records dropped since the last `advance()` call (RFC 053).
    /// Reset to 0 on each `advance()` and returned to the caller.
    drops_since_advance: Cell<u64>,
    /// RFC-0.41-001: the task parked in `sys_audit_wait`, if any. One slot: there
    /// is one ring and it has one consumer.
    waiter: Cell<Option<TaskId>>,
    /// RFC-0.41-001: a parked waiter that an append has claimed and that the trap
    /// path has not yet made runnable. See [`AuditRing::take_wake`].
    wake: Cell<Option<TaskId>>,
}

/// What `sys_audit_wait` should do, decided by [`AuditRing::park`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParkOutcome {
    /// The ring already holds records: return at once (the lost-wakeup guard).
    NotEmpty,
    /// Another live task is parked; the ring has one consumer.
    Busy,
    /// Registered as the waiter: block the caller.
    Parked,
}

// SAFETY: category=kernel-global-mutable single-hart.
unsafe impl Sync for AuditRing {}

impl AuditRing {
    const fn new() -> Self {
        const EMPTY: Cell<Option<AuditRecord>> = Cell::new(None);
        AuditRing {
            records: [EMPTY; AUDIT_RING_CAPACITY],
            head: Cell::new(0),
            len: Cell::new(0),
            seq: Cell::new(0),
            dropped_count: Cell::new(0),
            drops_since_advance: Cell::new(0),
            waiter: Cell::new(None),
            wake: Cell::new(None),
        }
    }

    /// Append an event.  Drops silently (incrementing drop counters) when full.
    pub fn lock_free_append(
        &self,
        kind: AuditKindInternal,
        arg0: usize,
        arg1: usize,
        result: isize,
    ) {
        // R7: filtered at the append site, so a filtered record never takes a slot.
        if kind == AuditKindInternal::TaskSwitch && !RECORD_TASK_SWITCH {
            return;
        }
        let len = self.len.get();
        if len >= AUDIT_RING_CAPACITY {
            self.dropped_count.set(self.dropped_count.get() + 1);
            self.drops_since_advance
                .set(self.drops_since_advance.get() + 1);
            return;
        }
        let seq = self.seq.get();
        let idx = (self.head.get() + len) % AUDIT_RING_CAPACITY;
        self.records[idx].set(Some(AuditRecord {
            seq,
            tick: 0,
            kind,
            arg0,
            arg1,
            result,
        }));
        self.seq.set(seq + 1);
        self.len.set(len + 1);
        // R3: claim a parked waiter. This is all the append path does about it --
        // it records that the waiter is owed a wake. Marking the task runnable and
        // enqueueing it (the two operations the IRQ path performs) cannot be done
        // from here: this is called from inside code that already holds `&mut` to
        // the task table and scheduler, and reaching them again would alias them.
        // The trap path performs those two operations at the one point where it
        // holds both and nothing else does -- see `trap::dispatch::deliver_audit_wake`.
        if let Some(w) = self.waiter.take() {
            self.wake.set(Some(w));
        }
    }

    /// `sys_audit_wait`'s decision, made atomically (single hart, in a trap).
    ///
    /// **The lost-wakeup guard is this check, not a latch.** The condition a waiter
    /// waits for is the ring's own state -- non-empty -- so a record appended in
    /// the window between "a drain returned empty" and "the task called
    /// `sys_audit_wait`" needs no remembered event: the wait finds the ring
    /// non-empty and returns at once. The IRQ path needs a latch because an
    /// interrupt is an edge; a ring that is not empty is a level.
    ///
    /// `stale(w)` says whether an already-registered waiter `w` is no longer
    /// parked (it died, or was woken by something else), in which case its
    /// registration is replaced rather than reported as `Busy`.
    pub fn park(&self, who: TaskId, stale: impl Fn(TaskId) -> bool) -> ParkOutcome {
        if self.len.get() > 0 {
            return ParkOutcome::NotEmpty;
        }
        if let Some(w) = self.waiter.get() {
            if w != who && !stale(w) {
                return ParkOutcome::Busy;
            }
        }
        self.waiter.set(Some(who));
        ParkOutcome::Parked
    }

    /// The waiter an append has claimed, if any; clears the claim.
    pub fn take_wake(&self) -> Option<TaskId> {
        self.wake.take()
    }

    /// Peek at record at logical index `i` from the head without consuming it.
    ///
    /// RFC 053: used by the peek-copy-advance drain loop so records are not
    /// removed from the ring until the user-space copy succeeds.
    /// Returns `None` if `i >= len`.
    pub fn peek_at(&self, i: usize) -> Option<AuditRecord> {
        if i >= self.len.get() {
            return None;
        }
        let idx = (self.head.get() + i) % AUDIT_RING_CAPACITY;
        self.records[idx].get()
    }

    /// Advance the ring head by `n` records, freeing those slots.
    ///
    /// RFC 053: called once after `n` successful user-space copies.
    /// Returns the per-drain drop count (drops since previous `advance`);
    /// resets that counter to 0.
    pub fn advance(&self, n: usize) -> u64 {
        if n > 0 {
            let new_len = self.len.get().saturating_sub(n);
            let new_head = (self.head.get() + n) % AUDIT_RING_CAPACITY;
            self.head.set(new_head);
            self.len.set(new_len);
        }
        let dropped = self.drops_since_advance.get();
        self.drops_since_advance.set(0);
        dropped
    }

    /// Read a record by logical index (0 = oldest in ring).
    #[allow(dead_code)] // internal helper; peek_at inlines this
    pub fn get(&self, index: usize) -> Option<AuditRecord> {
        if index >= self.len.get() {
            return None;
        }
        let idx = (self.head.get() + index) % AUDIT_RING_CAPACITY;
        self.records[idx].get()
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.len.get()
    }
    #[allow(dead_code)] // diagnostic accessor
    pub fn dropped(&self) -> u64 {
        self.dropped_count.get()
    }

    /// Number of records not yet drained (available for the next drain call).
    #[allow(dead_code)]
    pub fn pending(&self) -> usize {
        self.len.get() // drain_cursor removed (always 0 after drain_into removed in v0.2.22)
    }
}

// drain_into and compact removed in v0.2.22: fully superseded by peek_at + advance (RFC 053).
