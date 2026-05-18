//! Fixed-capacity, append-only audit ring.
//!
//! # Drain cursor
//! `sys_audit_drain` advances `drain_cursor` after copying records to user
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
    records:       [Cell<Option<AuditRecord>>; AUDIT_RING_CAPACITY],
    /// Index of the OLDEST record in `records` (modular).
    head:          Cell<usize>,
    /// Number of records currently in the ring (pending or already drained).
    len:           Cell<usize>,
    /// Next sequence number to assign.
    seq:           Cell<u64>,
    /// Total records dropped because the ring was full.
    dropped_count: Cell<u64>,
    /// Number of records (from the head) already consumed by `sys_audit_drain`.
    drain_cursor:  Cell<usize>,
}

// SAFETY: single-hart.
unsafe impl Sync for AuditRing {}

impl AuditRing {
    const fn new() -> Self {
        const EMPTY: Cell<Option<AuditRecord>> = Cell::new(None);
        AuditRing {
            records:       [EMPTY; AUDIT_RING_CAPACITY],
            head:          Cell::new(0),
            len:           Cell::new(0),
            seq:           Cell::new(0),
            dropped_count: Cell::new(0),
            drain_cursor:  Cell::new(0),
        }
    }

    /// Append an event.  Drops silently (incrementing `dropped_count`) when
    /// full and no drain has happened yet.
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
            return;
        }
        let seq = self.seq.get();
        let idx = (self.head.get() + len) % AUDIT_RING_CAPACITY;
        self.records[idx].set(Some(AuditRecord { seq, tick: 0, kind, arg0, arg1, result }));
        self.seq.set(seq + 1);
        self.len.set(len + 1);
    }

    /// Read a record by logical index (0 = oldest in ring).
    pub fn get(&self, index: usize) -> Option<AuditRecord> {
        if index >= self.len.get() { return None; }
        let idx = (self.head.get() + index) % AUDIT_RING_CAPACITY;
        self.records[idx].get()
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize  { self.len.get() }
    pub fn dropped(&self) -> u64 { self.dropped_count.get() }

    /// Number of records not yet drained (available for the next drain call).
    #[allow(dead_code)]
    pub fn pending(&self) -> usize {
        self.len.get().saturating_sub(self.drain_cursor.get())
    }

    /// Fill `out` with pending records starting from `drain_cursor`.
    ///
    /// Returns `(n_filled, n_dropped_total)`.  After filling, immediately
    /// calls [`Self::compact`] to reclaim the drained slots so the ring
    /// doesn't stay permanently full.
    pub fn drain_into(&self, out: &mut [AuditRecord]) -> (usize, u64) {
        let cursor  = self.drain_cursor.get();
        let pending = self.len.get().saturating_sub(cursor);
        let n       = pending.min(out.len());

        for i in 0..n {
            if let Some(rec) = self.get(cursor + i) {
                out[i] = rec;
            }
        }

        // Advance cursor then compact.
        self.drain_cursor.set(cursor + n);
        self.compact();

        (n, self.dropped_count.get())
    }

    /// Remove all drained records from the front of the ring, reclaiming
    /// capacity for future appends.
    fn compact(&self) {
        let drained = self.drain_cursor.get();
        if drained == 0 { return; }

        let new_len  = self.len.get().saturating_sub(drained);
        let new_head = (self.head.get() + drained) % AUDIT_RING_CAPACITY;
        self.head.set(new_head);
        self.len.set(new_len);
        self.drain_cursor.set(0);
    }
}
