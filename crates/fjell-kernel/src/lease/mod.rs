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

use fjell_abi::error::SysError;
use fjell_abi::lease::{LeaseEpoch, LeaseId};
use fjell_abi::task::TaskId;

// `MAX_LEASES` and `MAX_LEASES_PER_OWNER` live in `fjell_abi::lease` (RFC-0.48-001 D1 condition i): services
// derive their own bounds from them -- `neg-test`'s recycling scenario fills the table by the *capacity*, not by a
// second literal that would silently stop exercising reclamation the day the table grew.
pub use fjell_abi::lease::{MAX_LEASES, MAX_LEASES_PER_OWNER};

// The capacity must hold one boot lease for every task the kernel can have, plus one delegator's whole quota (so the
// broker can still grant when the system is as full of tasks as it can be), and `cursor` is a `u8`. Compile-time, so the
// next person to lower either constant is told here and not by an exhausted table at 03:00.
const _: () = assert!(
    MAX_LEASES >= crate::task::tcb::MAX_TASKS + MAX_LEASES_PER_OWNER,
    "MAX_LEASES must hold a boot lease per task plus one delegator's quota"
);
const _: () = assert!(MAX_LEASES <= 256, "the reclamation cursor is a u8");

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
    pub state: LeaseState,
    /// Slot generation, carried in the high half of a `LeaseId` and checked on every lookup.
    ///
    /// **Bumped when a `Revoked` slot is reclaimed** (RFC-0.46-001): a `LeaseId` issued for an earlier
    /// incarnation of this slot can then never name the current one. Until 0.46 no slot was ever reused
    /// and this stayed `0`. A slot at `u16::MAX` is retired for good, never reclaimed (retire-before-wrap,
    /// as the epoch already is). **This is the lease table's generation only: the task table's is a
    /// different field and is untouched** -- `TaskTable::remove` has no callers, so task generations are
    /// always `0`, and the kernel's `TaskId::new(x, 0)` idiom is correct *because* of that.
    pub generation: u16,
    /// Monotonically-increasing epoch counter; incremented on revoke.
    ///
    /// Starts at `1` (epoch `0` is reserved for "invalid/never issued").
    pub epoch: u32,
    /// The task that owns this lease (for lifecycle revoke).
    pub owner: TaskId,
    /// A **boot lease** (RFC-0.48-001): created by the kernel at spawn for the *subject's* capabilities, owned by its
    /// spawner so the spawner may revoke it. **Not ended by its owner's death** -- `revoke_owned_by` skips it -- because
    /// a supervisor exiting must not strip every service it started of its authority (that would be a new failure mode,
    /// and not the one this line adds); it ends when its *subject* exits, which the TCB's `boot_lease` records.
    pub boot: bool,
}

impl LeaseObject {
    const fn empty() -> Self {
        LeaseObject {
            state: LeaseState::Empty,
            generation: 0,
            epoch: 0,
            owner: TaskId {
                index: 0,
                generation: 0,
            },
            boot: false,
        }
    }
}

/// Fixed-capacity lease table.
pub struct LeaseTable {
    slots: [LeaseObject; MAX_LEASES],
    /// Where the next reclamation scan starts (RFC-0.46-001 R2): one past the last allocation.
    cursor: u8,
}

impl LeaseTable {
    pub const fn new() -> Self {
        LeaseTable {
            slots: [const { LeaseObject::empty() }; MAX_LEASES],
            cursor: 0,
        }
    }

    // ── Public API ────────────────────────────────────────────────────────

    /// Create a new lease owned by `owner`.
    ///
    /// RFC 033 §2.3: epoch starts at `1` for a never-used slot. RFC-0.46-001:
    ///
    /// * **R-Q** -- an owner already holding [`MAX_LEASES_PER_OWNER`] `Active` leases is refused
    ///   `LeaseQuotaExceeded`, so no one task can take the table from everyone else;
    /// * **R-R** -- the slot is a never-used one if any exists, otherwise a `Revoked` one is reclaimed
    ///   (round-robin from a cursor: the longest-revoked first). Reclamation is **lazy, on allocation
    ///   pressure, not in `revoke`**: a capability bound to a lease that has just been revoked must still
    ///   answer `LeaseRevoked`, which an eager reclaim would turn into a race with the next `create`.
    ///   The generation is bumped; **the epoch continues** (it is not reset), so a binding from the old
    ///   incarnation differs in epoch *and* generation;
    /// * neither exists -> `LeaseTableFull`, distinct from the quota (the system is out, not this owner).
    pub fn create(&mut self, owner: TaskId, _flags: u32) -> Result<LeaseId, SysError> {
        use fjell_abi::lease::lease_quota_ok;
        let owned = self
            .slots
            .iter()
            .filter(|s| s.state == LeaseState::Active && s.owner == owner)
            .count();
        if !lease_quota_ok(owned, MAX_LEASES_PER_OWNER) {
            return Err(SysError::LeaseQuotaExceeded);
        }
        self.allocate(owner, false)
    }

    /// Create the **boot lease** of a task the kernel is spawning on `owner`'s behalf (RFC-0.48-001 R2).
    ///
    /// **Not governed by [`MAX_LEASES_PER_OWNER`], deliberately and only here.** The quota bounds what a task *asks
    /// for* (`sys_lease_create`; the abandon-loop defence): one owner may not take the table from everyone else. A boot
    /// lease is not a request -- `init` would own thirty of them against a quota of sixteen, and charging them would make
    /// the seventeenth service unspawnable. What bounds boot leases instead is **`MAX_TASKS`** (a task has one) and the
    /// table's capacity (a compile-time assert holds one per task plus a delegator's whole quota). A full table is
    /// `LeaseTableFull`, and the spawn fails before it has taken any frame.
    pub fn create_boot(&mut self, owner: TaskId) -> Result<LeaseId, SysError> {
        self.allocate(owner, true)
    }

    /// The allocation both entries share: a never-used slot, else the longest-revoked reclaimable one.
    fn allocate(&mut self, owner: TaskId, boot: bool) -> Result<LeaseId, SysError> {
        use fjell_abi::lease::{Alloc, SlotKind, SlotView, lease_alloc};
        let mut views = [SlotView {
            kind: SlotKind::Empty,
            generation: 0,
            epoch: 0,
        }; MAX_LEASES];
        for (v, s) in views.iter_mut().zip(self.slots.iter()) {
            *v = SlotView {
                kind: match s.state {
                    LeaseState::Empty => SlotKind::Empty,
                    LeaseState::Active => SlotKind::Active,
                    LeaseState::Revoked => SlotKind::Revoked,
                },
                generation: s.generation,
                epoch: s.epoch,
            };
        }
        let (i, recycled) = match lease_alloc(&views, self.cursor as usize) {
            Alloc::Fresh(i) => (i, false),
            Alloc::Recycle(i) => (i, true),
            Alloc::Full => return Err(SysError::LeaseTableFull),
        };
        let slot = &mut self.slots[i];
        if recycled {
            // `lease_can_recycle` guaranteed headroom: no wrap.
            slot.generation += 1;
            // The epoch continues from its post-revocation value (>= 2), never back to 1.
        } else {
            slot.epoch = 1; // RFC 033: start at 1, not 0
        }
        slot.state = LeaseState::Active;
        slot.owner = owner;
        slot.boot = boot;
        self.cursor = ((i + 1) % MAX_LEASES) as u8;
        Ok(LeaseId::new(i as u16, slot.generation))
    }

    /// Return the current epoch for the given lease, whatever its state.
    ///
    /// A `Revoked` lease is reported with its post-revocation epoch — this is
    /// what `sys_lease_inspect` wants. To *bind* a capability use
    /// [`Self::active_epoch`], which refuses a lease that is not `Active`.
    pub fn current_epoch(&self, id: LeaseId) -> Result<LeaseEpoch, SysError> {
        Ok(LeaseEpoch(self.get(id)?.epoch))
    }

    /// Return the current epoch for the given lease only while it is `Active`.
    ///
    /// `LeaseRevoked` for a `Revoked` lease (the same error `check_active`
    /// gives a use of such a binding); `InvalidCap` for a lease that does not
    /// exist. Used by `sys_cap_bind_lease` (RFC-0.38-002): a binding stamped
    /// against a `Revoked` lease can never pass `check_active` again, so
    /// accepting one would only hide the revocation from the binder.
    pub fn active_epoch(&self, id: LeaseId) -> Result<LeaseEpoch, SysError> {
        let slot = self.get_bound(id)?;
        if slot.state != LeaseState::Active {
            return Err(SysError::LeaseRevoked);
        }
        Ok(LeaseEpoch(slot.epoch))
    }

    /// The task that created lease `id`, whatever the lease's state.
    ///
    /// `InvalidCap` for a lease that does not exist. Used by `sys_cap_bind_lease`
    /// (RFC-0.42-001 R2): a task binds only leases it created.
    pub fn owner(&self, id: LeaseId) -> Result<TaskId, SysError> {
        Ok(self.get(id)?.owner)
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
        // O(1) revocation via the shared bounded model (architect C6,
        // retire-before-wrap): the epoch never wraps. At u32::MAX the slot is
        // retired permanently — state stays Revoked and the epoch stays at
        // MAX, so every epoch-bound capability keeps failing. (Unreachable in
        // practice: it requires 2^32 revocations of one slot.)
        //
        // History, kept so the claim is not quietly re-added or re-removed: this note used to add
        // "slot reuse bumps the generation, which invalidates old LeaseIds independently" as a second
        // ground. That was FALSE until 0.46 -- no slot was reused and the generation was never
        // incremented -- and it was removed on 2026-10-21 rather than left propping up the argument.
        // Since RFC-0.46-001 it is TRUE: `create` reclaims a revoked slot and bumps the generation, and
        // the Verus model proves it independently of the epoch (LEASE-VERUS-008).
        match fjell_abi::lease::lease_revoke(slot.epoch) {
            fjell_abi::lease::RevokeOutcome::Advanced(e) => slot.epoch = e,
            fjell_abi::lease::RevokeOutcome::MustRetire => {
                // Epoch reached u32::MAX — retire the slot permanently rather
                // than wrapping (C6, LEASE-VERUS-005). Practically unreachable
                // in any deployment lifetime; logged for observability.
                // Architect follow-up (v0.18 review §6.5 / §8.2.4).
                crate::kprintln!("lease: slot {} retired before wrap (epoch=MAX)", id.0);
            }
        }
        slot.state = LeaseState::Revoked;
        let new_epoch = slot.epoch;
        // slot borrow ends here; wake_or_cancel receives the epoch directly so
        // it has no reason to call get_lease_table() and alias &mut self.
        wake_or_cancel_blocked_ipc_for_lease(id, new_epoch);
        Ok(LeaseEpoch(new_epoch))
    }

    /// Check that the lease `id` is still active with the given `bound_epoch`.
    ///
    /// Used by `check_lease` → `require_cap` step 7.
    pub fn check_active(&self, id: LeaseId, bound_epoch: LeaseEpoch) -> Result<(), SysError> {
        let slot = self.get_bound(id)?;
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
            // A boot lease belongs to its subject's lifetime, not its owner's (see `LeaseObject::boot`).
            if self.slots[i].state == LeaseState::Active
                && self.slots[i].owner == task
                && !self.slots[i].boot
            {
                let id = LeaseId::new(i as u16, self.slots[i].generation);
                let _ = self.revoke(id); // ignore error from already-revoked
            }
        }
    }

    // ── Internal helpers ──────────────────────────────────────────────────

    /// **Two mechanisms, one property, and they are a pair -- do not remove either.** A binding from an
    /// earlier incarnation of a reused slot is dead for two independent reasons: its generation is older
    /// (this function) *and* its epoch differs from the new incarnation's, because `create` continues the
    /// epoch instead of restarting it. **While the epoch continues, the generation clause here is
    /// runtime-redundant**: mutating it away (L3, "stale ids accepted as current") stays green on a real
    /// boot, and that green is this redundancy, not a gap. Only removing *both* (L3b: stale accepted and
    /// the epoch restarted at 1) is red. What makes the generation *necessary* is the Verus model
    /// (`verification/verus/lease/lease_epoch.rs`, LEASE-VERUS-008/-009), which proves it for **any** epoch
    /// the new incarnation starts at -- so it must survive a future change that lets an epoch collide.
    /// Keep both; a reader who finds two mechanisms where one suffices will delete whichever looks more
    /// incidental.
    ///
    /// `get` for the *use* of a binding (`check_active`, `active_epoch`): a `LeaseId` that names an
    /// **earlier incarnation** of a reclaimed slot is `LeaseRevoked` -- the lease that capability was
    /// bound to is gone, which is exactly what it answered a moment before the slot was reclaimed
    /// (RFC-0.46-001 R5). An id from a future incarnation, or past the table, was never issued:
    /// `InvalidCap`, as `get` says.
    fn get_bound(&self, id: LeaseId) -> Result<&LeaseObject, SysError> {
        use fjell_abi::lease::{IdRelation, lease_id_relation};
        let slot = self
            .slots
            .get(id.index() as usize)
            .ok_or(SysError::InvalidCap)?;
        if slot.state == LeaseState::Empty {
            return Err(SysError::InvalidCap);
        }
        match lease_id_relation(slot.generation, id.generation()) {
            IdRelation::Current => Ok(slot),
            IdRelation::Stale => Err(SysError::LeaseRevoked),
            IdRelation::Forged => Err(SysError::InvalidCap),
        }
    }

    fn get(&self, id: LeaseId) -> Result<&LeaseObject, SysError> {
        let slot = self
            .slots
            .get(id.index() as usize)
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
fn wake_or_cancel_blocked_ipc_for_lease(id: LeaseId, new_epoch: u32) {
    // SAFETY: category=kernel-global-mutable
    //   Single-hart kernel; all global pointers are exclusively owned in this call.
    let (tasks, sched, ct, et) = unsafe { crate::get_kernel_state() };

    // new_epoch was passed in by revoke() immediately after the wrapping_add.
    // old_epoch is the epoch that in-flight IPC waiters stored at issue time.
    // No second get_lease_table() call — that would alias the &mut self held by
    // the revoke() caller (RFC-v0.7.4-003 implementation note).
    let old_epoch: u32 = new_epoch.wrapping_sub(1);

    // RFC 050 / architect review v0.19 (recorded finding, fixed v0.20):
    // a lease revocation must also cancel server-side reply edges whose
    // lease binding matches the revoked epoch, and wake the blocked callers.
    // Previously only endpoint sendq/recvq waiters were cancelled, so a
    // caller blocked awaiting a reply was never woken (caller hang) and the
    // server's later sys_ipc_reply met a live-but-stale edge instead of the
    // contract-specified BadState.
    {
        let (cancelled_callers, n_cancelled) = ct.cancel_replies_for_lease(id, old_epoch);
        for &tid in cancelled_callers.iter().take(n_cancelled) {
            let task_id = fjell_abi::task::TaskId::new(tid, 0);
            // Same terminal-state guard as the endpoint-queue wakes below:
            // never re-enqueue Exited/Faulted tasks.
            let task_is_wakeable = matches!(
                tasks.get(task_id).map(|t| &t.state),
                Some(crate::task::tcb::TaskState::Blocked(_))
            );
            if task_is_wakeable {
                if let Some(task) = tasks.get_mut(task_id) {
                    task.trap_frame.gpr[crate::task::tcb::REG_A0] =
                        fjell_abi::error::SysError::LeaseRevoked as isize as usize;
                    task.state = crate::task::tcb::TaskState::Runnable;
                    sched.enqueue_runnable(task_id, 128);
                }
            }
        }
    }

    // Walk every endpoint by ID and cancel matching waiters.
    // We iterate IDs first to avoid a mutable borrow conflict between 'et'
    // and 'tasks'/'sched' (all come from the same get_kernel_state() call).
    let ep_ids: [u32; crate::cap::table::MAX_ENDPOINTS] = {
        let mut arr = [0u32; crate::cap::table::MAX_ENDPOINTS];
        let mut count = 0usize;
        for id_val in et.iter_allocated_ids() {
            if count < arr.len() {
                arr[count] = id_val;
                count += 1;
            }
        }
        arr
    };

    for &ep_id in ep_ids.iter() {
        let ep = match et.get_mut(ep_id) {
            Some(e) => e,
            None => continue,
        };
        let cancelled = ep.cancel_by_lease(id, old_epoch);

        // Wake cancelled senders.
        for i in 0..cancelled.n_senders {
            let tid = cancelled.sender_tids[i];
            // Safety note: CancelledSet holds raw u16 indices (no generation).
            // We probe the task table with generation=0 (the most common first
            // generation). If the task has been replaced with a newer generation,
            // get_mut returns None and we skip — the new occupant is unrelated.
            //
            // CRITICAL: we must NOT call sched.enqueue_runnable for tasks we
            // cannot find.  Enqueuing a stale TaskId::new(idx, 0) can reschedule
            // an already-exited task whose slot now holds generation=0 for a new
            // service.  That service would resume from its saved PC into BSS
            // (zeros), decode zeros as `lb x0, 0(x0)`, and fault.
            let task_id = fjell_abi::task::TaskId::new(tid, 0);
            // Only wake a task if it is in a terminal-safe state.
            // Exited and Faulted tasks must never be re-enqueued — they would
            // resume from a stale PC into BSS (zeros), causing LoadPageFault.
            let task_is_wakeable = matches!(
                tasks.get(task_id).map(|t| &t.state),
                Some(crate::task::tcb::TaskState::Blocked(_))
            );
            if task_is_wakeable {
                if let Some(task) = tasks.get_mut(task_id) {
                    task.trap_frame.gpr[crate::task::tcb::REG_A0] =
                        fjell_abi::error::SysError::LeaseRevoked as isize as usize;
                    task.state = crate::task::tcb::TaskState::Runnable;
                    sched.enqueue_runnable(task_id, 128);
                }
            }
        }
        // Wake cancelled receivers.
        for i in 0..cancelled.n_receivers {
            let tid = cancelled.receiver_tids[i];
            let task_id = fjell_abi::task::TaskId::new(tid, 0);
            let task_is_wakeable = matches!(
                tasks.get(task_id).map(|t| &t.state),
                Some(crate::task::tcb::TaskState::Blocked(_))
            );
            if task_is_wakeable {
                if let Some(task) = tasks.get_mut(task_id) {
                    task.trap_frame.gpr[crate::task::tcb::REG_A0] =
                        fjell_abi::error::SysError::LeaseRevoked as isize as usize;
                    task.state = crate::task::tcb::TaskState::Runnable;
                    sched.enqueue_runnable(task_id, 128);
                }
            }
        }
    }

    // Emit a pinned-critical audit event for every revoke that affects waiters.
    // (The audit call is best-effort; failure here must not abort the revoke.)
    crate::audit::ring::AUDIT.lock_free_append(
        crate::audit::ring::AuditKindInternal::LeaseRevoked,
        id.0 as usize,
        new_epoch as usize,
        0,
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
        id: fjell_abi::lease::LeaseId,
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

    fn owner() -> TaskId {
        TaskId::new(1, 0)
    }

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
