//! Kernel lease table — RFC 033 lease epoch revocation (v0.2.0 Phase 2).
//!
//! # Design principles (RFC 033)
//!
//! 1. Revocation is O(1): `lease.epoch += 1`.
//! 2. No capability table is walked on revoke.
//! 3. Capabilities fail on next use (lazy invalidation).
//! 4. Recursive policy revoke belongs to cap-broker.
//!
//! # Invariants
//!
//! - LEASE-001  Lease revoke is O(1) in kernel.
//! - LEASE-002  Revoked lease invalidates all lease-bound capabilities.
//! - LEASE-003  Recursive revocation is NOT in kernel.
//! - LEASE-004  cap-broker owns policy-level revoke trees.
//! - LEASE-005  cap_drop remains possible for revoked capabilities.
//! - LEASE-006  Lease epoch mismatch always rejects capability use.

use fjell_abi::lease::{LeaseEpoch, LeaseId};
use fjell_abi::task::TaskId;
use fjell_abi::error::SysError;

pub const MAX_LEASES: usize = 32;

/// Lifecycle state of a lease object.
///
/// Matches RFC 033 §2.1 `LeaseState`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LeaseState {
    /// Slot available for allocation.
    Empty,
    /// Lease is in use and has not been revoked.
    Active,
    /// Lease has been revoked; epoch was incremented.
    ///
    /// All capabilities whose `epoch_at_issue` != current epoch fail use.
    Revoked,
}

/// A single lease object.
///
/// Matches RFC 033 §2.1 `LeaseObject`.
struct LeaseObject {
    pub state:      LeaseState,
    /// Slot generation — incremented when the slot is freed.
    pub generation: u16,
    /// Monotonically-increasing epoch counter; incremented on revoke.
    ///
    /// Starts at `1` (epoch `0` is reserved for "invalid/never issued").
    pub epoch:      u32,
    /// The task that owns this lease (for lifecycle revoke).
    pub owner:      TaskId,
}

impl LeaseObject {
    const fn empty() -> Self {
        LeaseObject {
            state:      LeaseState::Empty,
            generation: 0,
            epoch:      0,
            owner:      TaskId { index: 0, generation: 0 },
        }
    }
}

/// Fixed-capacity lease table.
pub struct LeaseTable {
    slots: [LeaseObject; MAX_LEASES],
}

impl LeaseTable {
    pub const fn new() -> Self {
        LeaseTable { slots: [const { LeaseObject::empty() }; MAX_LEASES] }
    }

    // ── Public API ────────────────────────────────────────────────────────

    /// Create a new lease owned by `owner`.
    ///
    /// RFC 033 §2.3: epoch starts at `1`.
    pub fn create(&mut self, owner: TaskId, _flags: u32) -> Result<LeaseId, SysError> {
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if slot.state == LeaseState::Empty {
                slot.state = LeaseState::Active;
                slot.epoch = 1;   // RFC 033: start at 1, not 0
                slot.owner = owner;
                return Ok(LeaseId::new(i as u16, slot.generation));
            }
        }
        Err(SysError::NoMemory)
    }

    /// Return the current epoch for the given lease.
    pub fn current_epoch(&self, id: LeaseId) -> Result<LeaseEpoch, SysError> {
        Ok(LeaseEpoch(self.get(id)?.epoch))
    }

    /// Revoke a lease: O(1) epoch increment (RFC 033 §2.4, invariant LEASE-001).
    ///
    /// Returns the *new* epoch after revocation.
    ///
    /// After this call:
    /// - All capabilities whose `epoch_at_issue` matches the *old* epoch fail.
    /// - `wake_or_cancel_blocked_ipc_for_lease` is called (RFC 034 hook).
    pub fn revoke(&mut self, id: LeaseId) -> Result<LeaseEpoch, SysError> {
        let idx = id.index() as usize;
        let slot = self.slots.get_mut(idx).ok_or(SysError::InvalidCap)?;
        if slot.generation != id.generation() {
            return Err(SysError::InvalidCap);
        }
        if slot.state == LeaseState::Empty {
            return Err(SysError::InvalidCap);
        }
        // O(1) revocation: increment epoch, mark Revoked.
        slot.epoch = slot.epoch.wrapping_add(1);
        slot.state = LeaseState::Revoked;
        // RFC 034 hook: wake/cancel blocked IPC tasks waiting on this lease.
        // The full implementation is deferred to Phase 2 completion when
        // the blocked-IPC data structures are ready.  This call is a no-op
        // stub until then.
        wake_or_cancel_blocked_ipc_for_lease(id);
        Ok(LeaseEpoch(slot.epoch))
    }

    /// Check that the lease `id` is still active with the given `bound_epoch`.
    ///
    /// Used by `check_lease` → `require_cap` step 7.
    pub fn check_active(&self, id: LeaseId, bound_epoch: LeaseEpoch) -> Result<(), SysError> {
        let slot = self.get(id)?;
        if slot.state != LeaseState::Active {
            return Err(SysError::LeaseRevoked);
        }
        if slot.epoch != bound_epoch.0 {
            return Err(SysError::LeaseRevoked);
        }
        Ok(())
    }

    /// Lifecycle revoke: revoke all leases owned by `task` (RFC 033 §2.10).
    ///
    /// Called when a task exits, faults, or is restarted.  Each affected
    /// lease is revoked independently so blocked IPC is woken per lease.
    pub fn revoke_owned_by(&mut self, task: TaskId) {
        for i in 0..MAX_LEASES {
            if self.slots[i].state == LeaseState::Active
                && self.slots[i].owner == task
            {
                let id = LeaseId::new(i as u16, self.slots[i].generation);
                let _ = self.revoke(id);   // ignore error from already-revoked
            }
        }
    }

    // ── Internal helpers ──────────────────────────────────────────────────

    fn get(&self, id: LeaseId) -> Result<&LeaseObject, SysError> {
        let slot = self.slots.get(id.index() as usize)
            .ok_or(SysError::InvalidCap)?;
        if slot.generation != id.generation() || slot.state == LeaseState::Empty {
            return Err(SysError::InvalidCap);
        }
        Ok(slot)
    }
}

// ── RFC-v0.7.4-003 / W-H-02: unified lease-revocation IPC wake ────────────────

/// Wake or cancel any tasks blocked in IPC that are bound to `lease_id`.
///
/// RFC-v0.7.4-003 (closes W-H-02): implements the RFC 034 hook properly.
/// Walks every endpoint, cancels sender/receiver queue entries whose
/// lease epoch matches the revoked lease, then wakes the affected tasks.
///
/// This is O(MAX_ENDPOINTS × queue_depth) — acceptable for the current
/// MAX_ENDPOINTS=32 and QUEUE_DEPTH=8 (256 operations worst-case).
fn wake_or_cancel_blocked_ipc_for_lease(id: LeaseId) {
    // SAFETY: category=kernel-global-mutable
    //   Single-hart kernel; all global pointers are exclusively owned in this call.
    let (tasks, sched, _, et) = unsafe { crate::get_kernel_state() };

    // Retrieve the current epoch (just incremented by the revoke call above).
    // We use a local copy to avoid a second borrow of the lease table.
    let epoch = {
        // SAFETY: category=kernel-global-mutable  single-hart, no races.
        let lt_tmp = unsafe { crate::get_lease_table() };
        match lt_tmp.current_epoch(id) {
            Ok(e) => e.0,
            Err(_) => return,
        }
    };

    // Epoch before the revoke is epoch - 1 (the epoch in-flight tasks had).
    // cancel_by_lease checks if the stored epoch matches the old epoch.
    let old_epoch = epoch.wrapping_sub(1);

    // Walk every endpoint and cancel matching waiters.
    for (_ep_id, ep) in et.iter_allocated() {
        let cancelled = ep.cancel_by_lease(id, old_epoch);
        // Wake cancelled senders.
        for i in 0..cancelled.n_senders {
            let tid = cancelled.sender_tids[i];
            let task_id = crate::task::TaskId::new(tid, 0);
            if let Some(task) = tasks.get_mut(task_id) {
                // Deliver LeaseRevoked status to the sender's trap frame.
                task.trap_frame.gpr[crate::task::tcb::REG_A0] =
                    fjell_abi::error::SysError::LeaseRevoked as isize as usize;
                task.state = crate::task::tcb::TaskState::Runnable;
            }
            sched.enqueue(task_id, 128);
        }
        // Wake cancelled receivers.
        for i in 0..cancelled.n_receivers {
            let tid = cancelled.receiver_tids[i];
            let task_id = crate::task::TaskId::new(tid, 0);
            if let Some(task) = tasks.get_mut(task_id) {
                task.trap_frame.gpr[crate::task::tcb::REG_A0] =
                    fjell_abi::error::SysError::LeaseRevoked as isize as usize;
                task.state = crate::task::tcb::TaskState::Runnable;
            }
            sched.enqueue(task_id, 128);
        }
    }

    // Emit a pinned-critical audit event for every revoke that affects waiters.
    // (The audit call is best-effort; failure here must not abort the revoke.)
    crate::audit::ring::AUDIT.lock_free_append(
        crate::audit::ring::AuditKindInternal::LeaseRevoked,
        id.0 as usize, epoch as usize, 0,
    );
}

// ── RFC 006 + RFC 033: implement LeaseChecker for the kernel lease table ──────

impl fjell_cap::slot::LeaseChecker for LeaseTable {
    /// Check that the lease identified by `id` is still active at `epoch_issued`.
    ///
    /// Returns `Ok(())` if active; `Err(CapError::LeaseRevoked)` otherwise.
    /// This feeds into `require_cap()` step 7 (lease check).
    fn check_active(
        &self,
        id:           fjell_abi::lease::LeaseId,
        epoch_issued: fjell_abi::lease::LeaseEpoch,
    ) -> Result<(), fjell_cap::CapError> {
        self.check_active(id, epoch_issued)
            .map_err(|_| fjell_cap::CapError::LeaseRevoked)
    }
}

// ── Host-side unit tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use fjell_abi::task::TaskId;

    fn owner() -> TaskId { TaskId::new(1, 0) }

    #[test]
    fn lease_create_starts_at_epoch_one() {
        let mut lt = LeaseTable::new();
        let id = lt.create(owner(), 0).unwrap();
        assert_eq!(lt.current_epoch(id).unwrap().0, 1);
    }

    #[test]
    fn check_active_accepts_correct_epoch() {
        let mut lt = LeaseTable::new();
        let id = lt.create(owner(), 0).unwrap();
        let ep = lt.current_epoch(id).unwrap();
        lt.check_active(id, ep).unwrap();
    }

    #[test]
    fn revoke_increments_epoch() {
        let mut lt = LeaseTable::new();
        let id = lt.create(owner(), 0).unwrap();
        let old_ep = lt.current_epoch(id).unwrap();
        lt.revoke(id).unwrap();
        let new_ep = lt.current_epoch(id).unwrap();
        assert_eq!(new_ep.0, old_ep.0 + 1);
    }

    #[test]
    fn check_active_rejects_old_epoch() {
        let mut lt = LeaseTable::new();
        let id = lt.create(owner(), 0).unwrap();
        let old_ep = lt.current_epoch(id).unwrap();
        lt.revoke(id).unwrap();
        // Old epoch must now fail.
        assert!(lt.check_active(id, old_ep).is_err());
    }

    #[test]
    fn check_active_rejects_revoked_state() {
        let mut lt = LeaseTable::new();
        let id = lt.create(owner(), 0).unwrap();
        lt.revoke(id).unwrap();
        let new_ep = lt.current_epoch(id).unwrap();
        // Even with the current epoch, a revoked lease fails.
        // (state is Revoked; only Active passes)
        assert!(lt.check_active(id, new_ep).is_err());
    }

    #[test]
    fn lifecycle_revoke_revokes_owned_leases() {
        let mut lt = LeaseTable::new();
        let task = TaskId::new(3, 0);
        let id1 = lt.create(task, 0).unwrap();
        let id2 = lt.create(task, 0).unwrap();
        let ep1 = lt.current_epoch(id1).unwrap();
        let ep2 = lt.current_epoch(id2).unwrap();
        lt.revoke_owned_by(task);
        assert!(lt.check_active(id1, ep1).is_err());
        assert!(lt.check_active(id2, ep2).is_err());
    }

    #[test]
    fn stale_lease_id_rejected() {
        let mut lt = LeaseTable::new();
        let id = lt.create(owner(), 0).unwrap();
        // Forge a handle with wrong generation.
        let wrong_id = LeaseId::new(id.index(), id.generation().wrapping_add(1));
        assert!(lt.check_active(wrong_id, LeaseEpoch(1)).is_err());
    }
}
