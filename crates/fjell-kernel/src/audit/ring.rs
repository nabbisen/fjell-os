//! Fixed-capacity, append-only audit ring.
//!
//! Read methods (get/len/dropped) are unused in M2 but required by M5 auditd.
#![allow(dead_code)]

use core::cell::Cell;

/// Maximum number of events stored in the kernel ring.
pub const AUDIT_RING_CAPACITY: usize = 256;

/// Internal audit event kind (kernel-private; mapped to `fjell-audit-format`
/// types before export to user space).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditKindInternal {
    Boot,
    VmMap,
    VmFault,
    TaskCreate,
    TaskSwitch,
    TaskExit,
    TaskFault,
    Syscall,
    UnknownSyscall,
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

/// Append-only fixed-capacity audit ring.
///
/// # Thread-safety
/// Single-hart M2: `Cell` is used for interior mutability without locking.
/// In M3+ (SMP) this must be replaced with a spinlock or lock-free structure.
pub struct AuditRing {
    records:       [Cell<Option<AuditRecord>>; AUDIT_RING_CAPACITY],
    head:          Cell<usize>,
    len:           Cell<usize>,
    seq:           Cell<u64>,
    dropped_count: Cell<u64>,
}

// SAFETY: single-hart M2.
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
        }
    }

    /// Append an event.  Drops silently (incrementing `dropped_count`) when full.
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

    /// Read a record by index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<AuditRecord> {
        if index >= self.len.get() { return None; }
        let idx = (self.head.get() + index) % AUDIT_RING_CAPACITY;
        self.records[idx].get()
    }

    pub fn len(&self) -> usize { self.len.get() }
    pub fn dropped(&self) -> u64 { self.dropped_count.get() }
}
