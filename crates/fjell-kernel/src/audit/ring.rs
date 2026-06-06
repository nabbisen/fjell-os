//! Fixed-capacity, append-only audit ring.
//!
//! # Drain cursor
//! `sys_audit_drain` uses `peek_at` + `advance` (RFC 053) — no drain cursor needed.
//! space.  After draining, [`AuditRing::compact`] is called to reclaim slots
//! from the front of the ring so the ring never gets permanently stuck when
//! full.

use core::cell::Cell;

/// Maximum number of events stored in the kernel ring.
pub const AUDIT_RING_CAPACITY: usize = 256;

/// Internal audit event kind (kernel-private; mapped to `fjell-audit-format`
/// types before export to user space).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
#[repr(u16)]
pub enum AuditKindInternal {
    Boot            = 0,
    VmMap           = 1,
    VmFault         = 2,
    TaskCreate      = 3,
    TaskSwitch      = 4,
    TaskExit        = 5,
    TaskFault       = 6,
    Syscall         = 7,
    UnknownSyscall  = 8,
    // M3 capability events
    CapCopy         = 10,
    CapMint         = 11,
    CapDelete       = 12,
    CapRevoke       = 13,
    /// RFC 032 (v0.2.0): explicit slot drop via sys_cap_drop.
    CapDrop         = 15,
    // M3 IPC events
    IpcSend         = 20,
    IpcRecv         = 21,
    IpcCall         = 22,
    IpcReply        = 23,
    IpcDenied       = 24,
    /// RFC 037 (v0.2.0): task consumed its quantum without voluntary yield.
    TaskQuantumExceeded = 30,
    // Lease events (RFC-v0.7.4-003 / W-H-02)
    LeaseRevoked   = 40,
    CapDenied      = 41,
}

/// A single audit record stored in the ring.
#[derive(Clone, Copy)]
pub struct AuditRecord {
    pub seq:    u64,
    pub tick:   u64,
    pub kind:   AuditKindInternal,
    pub arg0:   usize,
    pub arg1:   usize,
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
    records:              [Cell<Option<AuditRecord>>; AUDIT_RING_CAPACITY],
    /// Index of the OLDEST record in `records` (modular).
    head:                 Cell<usize>,
    /// Number of records currently in the ring.
    len:                  Cell<usize>,
    /// Next sequence number to assign.
    seq:                  Cell<u64>,
    /// Total records dropped (lifetime counter; never decremented).
    dropped_count:        Cell<u64>,
    /// Records dropped since the last `advance()` call (RFC 053).
    /// Reset to 0 on each `advance()` and returned to the caller.
    drops_since_advance:  Cell<u64>,

}

// SAFETY: category=kernel-global-mutable single-hart.
unsafe impl Sync for AuditRing {}

impl AuditRing {
    const fn new() -> Self {
        const EMPTY: Cell<Option<AuditRecord>> = Cell::new(None);
        AuditRing {
            records:             [EMPTY; AUDIT_RING_CAPACITY],
            head:                Cell::new(0),
            len:                 Cell::new(0),
            seq:                 Cell::new(0),
            dropped_count:       Cell::new(0),
            drops_since_advance: Cell::new(0),
        }
    }

    /// Append an event.  Drops silently (incrementing drop counters) when full.
    pub fn lock_free_append(
        &self,
        kind:   AuditKindInternal,
        arg0:   usize,
        arg1:   usize,
        result: isize,
    ) {
        let len = self.len.get();
        if len >= AUDIT_RING_CAPACITY {
            self.dropped_count.set(self.dropped_count.get() + 1);
            self.drops_since_advance.set(self.drops_since_advance.get() + 1);
            return;
        }
        let seq = self.seq.get();
        let idx = (self.head.get() + len) % AUDIT_RING_CAPACITY;
        self.records[idx].set(Some(AuditRecord { seq, tick: 0, kind, arg0, arg1, result }));
        self.seq.set(seq + 1);
        self.len.set(len + 1);
    }

    /// Peek at record at logical index `i` from the head without consuming it.
    ///
    /// RFC 053: used by the peek-copy-advance drain loop so records are not
    /// removed from the ring until the user-space copy succeeds.
    /// Returns `None` if `i >= len`.
    pub fn peek_at(&self, i: usize) -> Option<AuditRecord> {
        if i >= self.len.get() { return None; }
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
            let new_len  = self.len.get().saturating_sub(n);
            let new_head = (self.head.get() + n) % AUDIT_RING_CAPACITY;
            self.head.set(new_head);
            self.len.set(new_len);
        }
        let dropped = self.drops_since_advance.get();
        self.drops_since_advance.set(0);
        dropped
    }

    /// Read a record by logical index (0 = oldest in ring).
    #[allow(dead_code)]  // internal helper; peek_at inlines this
    pub fn get(&self, index: usize) -> Option<AuditRecord> {
        if index >= self.len.get() { return None; }
        let idx = (self.head.get() + index) % AUDIT_RING_CAPACITY;
        self.records[idx].get()
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize  { self.len.get() }
    #[allow(dead_code)]  // diagnostic accessor
    pub fn dropped(&self) -> u64 { self.dropped_count.get() }

    /// Number of records not yet drained (available for the next drain call).
    #[allow(dead_code)]
    pub fn pending(&self) -> usize {
        self.len.get()  // drain_cursor removed (always 0 after drain_into removed in v0.2.22)
    }
}

// drain_into and compact removed in v0.2.22: fully superseded by peek_at + advance (RFC 053).
