//! Kernel lease table -- RFC 033 lease epoch revocation (v0.2.0 Phase 2).
//!
//! **The table itself is `fjell-lease`** (RFC-0.50-001), where its tests run on the host. What stays here is what touches the rest of
//! the kernel: the **wake** that follows every revocation (it dereferences the task, scheduler, capability and endpoint tables, which
//! is why `revoke` could not be tested while it lived in this crate), and the wrapper that makes the wake **the only revoke path the
//! kernel exposes** -- `LeaseTable` below has no way to revoke without it, because `fjell_lease::LeaseTable::revoke` returns what must
//! be woken and does not wake it.
//!
//! # Design principles (RFC 033)
//!
//! 1. Revocation is O(1): `lease.epoch += 1`.
//! 2. No capability table is walked on revoke.
//! 3. Capabilities fail on next use (lazy invalidation).
//! 4. Recursive policy revoke belongs to cap-broker.

use fjell_abi::error::SysError;
use fjell_abi::lease::{LeaseEpoch, LeaseId};
use fjell_abi::task::TaskId;
pub use fjell_abi::lease::{MAX_LEASES, MAX_LEASES_PER_OWNER};
use fjell_lease::Revoked;

// The capacity must hold one boot lease for every task the kernel can have, plus one delegator's whole quota (so the
// broker can still grant when the system is as full of tasks as it can be). Compile-time, so the next person to lower either
// constant is told here and not by an exhausted table at 03:00. (Stays in the kernel: `MAX_TASKS` is the task table's.)
const _: () = assert!(
    MAX_LEASES >= crate::task::tcb::MAX_TASKS + MAX_LEASES_PER_OWNER,
    "MAX_LEASES must hold a boot lease per task plus one delegator's quota"
);

/// The kernel's lease table: `fjell_lease::LeaseTable` plus the wake. **Every revoke goes through [`LeaseTable::revoke`] or
/// [`LeaseTable::revoke_owned_by`], and both wake.** (`Deref` is deliberately not implemented, so the non-waking revoke of the
/// inner table is not reachable from here.)
pub struct LeaseTable(fjell_lease::LeaseTable);

impl LeaseTable {
    pub const fn new() -> Self {
        LeaseTable(fjell_lease::LeaseTable::new())
    }

    pub fn create(&mut self, owner: TaskId, flags: u32) -> Result<LeaseId, SysError> {
        self.0.create(owner, flags)
    }

    pub fn create_boot(&mut self, owner: TaskId) -> Result<LeaseId, SysError> {
        self.0.create_boot(owner)
    }

    pub fn current_epoch(&self, id: LeaseId) -> Result<LeaseEpoch, SysError> {
        self.0.current_epoch(id)
    }

    pub fn active_epoch(&self, id: LeaseId) -> Result<LeaseEpoch, SysError> {
        self.0.active_epoch(id)
    }

    pub fn owner(&self, id: LeaseId) -> Result<TaskId, SysError> {
        self.0.owner(id)
    }

    pub fn check_active(&self, id: LeaseId, bound_epoch: LeaseEpoch) -> Result<(), SysError> {
        self.0.check_active(id, bound_epoch)
    }

    /// Revoke a lease (RFC 033 §2.4, LEASE-001) and **wake whoever was blocked on it** (RFC 034 hook). Returns the new epoch.
    pub fn revoke(&mut self, id: LeaseId) -> Result<LeaseEpoch, SysError> {
        let r = self.0.revoke(id)?;
        after_revoke(&r);
        Ok(r.epoch)
    }

    /// Lifecycle revoke: revoke every lease `task` owns that is not a boot lease (RFC 033 §2.10), waking per lease.
    /// Called when a task exits, faults, is destroyed, or restarted.
    pub fn revoke_owned_by(&mut self, task: TaskId) {
        loop {
            let batch = self.0.revoke_owned_by(task);
            for r in batch.iter() {
                after_revoke(r);
            }
            if !batch.more {
                break;
            }
        }
    }
}

/// What follows a revocation, and what `fjell_lease` could not do: say so if the slot was retired, then wake or cancel the
/// tasks blocked on it. `wake_or_cancel_blocked_ipc_for_lease` receives the epoch directly, so it has no reason to call
/// `get_lease_table()` and alias the `&mut self` the caller holds (RFC-v0.7.4-003).
fn after_revoke(r: &Revoked) {
    if r.retired {
        // Epoch reached u32::MAX: the slot is retired permanently rather than wrapping (C6, LEASE-VERUS-005). Practically
        // unreachable in any deployment lifetime; logged for observability (architect follow-up, v0.18 review §6.5 / §8.2.4).
        crate::kprintln!("lease: slot {} retired before wrap (epoch=MAX)", r.id.0);
    }
    wake_or_cancel_blocked_ipc_for_lease(r.id, r.epoch.0);
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
            // RFC-0.49-001 D3: `tid` is an index from a reply edge, and destroying a task removes every edge that names its
            // slot, so an index in a live edge always names the current occupant.
            // Same terminal-state guard as the endpoint-queue wakes below: never re-enqueue Exited/Faulted tasks.
            let task_is_wakeable = matches!(
                tasks.get_by_index(tid).map(|t| &t.state),
                Some(crate::task::tcb::TaskState::Blocked(_))
            );
            if task_is_wakeable {
                if let Some(task) = tasks.get_by_index_mut(tid) {
                    task.trap_frame.gpr[crate::task::tcb::REG_A0] =
                        fjell_abi::error::SysError::LeaseRevoked as isize as usize;
                    task.state = crate::task::tcb::TaskState::Runnable;
                    sched.enqueue_runnable(task.id, 128);
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
            // `CancelledSet` holds raw u16 indices (no generation). **That is sound because destroying a task purges every
            // endpoint queue entry that names its slot** (RFC-0.49-001 D5), so an index in a live queue names the current
            // occupant. (This used to be sound for the opposite reason -- a `TaskId::new(tid, 0)` lookup *missed* a replaced
            // slot -- which stopped being true the moment a slot could be reused at all.)
            //
            // Only wake a task if it is in a terminal-safe state. Exited and Faulted tasks must never be re-enqueued -- they
            // would resume from a stale PC into BSS (zeros), causing LoadPageFault.
            let task_is_wakeable = matches!(
                tasks.get_by_index(tid).map(|t| &t.state),
                Some(crate::task::tcb::TaskState::Blocked(_))
            );
            if task_is_wakeable {
                if let Some(task) = tasks.get_by_index_mut(tid) {
                    task.trap_frame.gpr[crate::task::tcb::REG_A0] =
                        fjell_abi::error::SysError::LeaseRevoked as isize as usize;
                    task.state = crate::task::tcb::TaskState::Runnable;
                    sched.enqueue_runnable(task.id, 128);
                }
            }
        }
        // Wake cancelled receivers.
        for i in 0..cancelled.n_receivers {
            let tid = cancelled.receiver_tids[i];
            let task_is_wakeable = matches!(
                tasks.get_by_index(tid).map(|t| &t.state),
                Some(crate::task::tcb::TaskState::Blocked(_))
            );
            if task_is_wakeable {
                if let Some(task) = tasks.get_by_index_mut(tid) {
                    task.trap_frame.gpr[crate::task::tcb::REG_A0] =
                        fjell_abi::error::SysError::LeaseRevoked as isize as usize;
                    task.state = crate::task::tcb::TaskState::Runnable;
                    sched.enqueue_runnable(task.id, 128);
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
