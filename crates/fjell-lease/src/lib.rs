//! The lease table -- RFC 033 lease epoch revocation (v0.2.0 Phase 2), extracted from the kernel (RFC-0.50-001).
//!
//! This was `crates/fjell-kernel/src/lease/mod.rs`. It moved, with its seven tests, because a test in a bare-metal crate is
//! never compiled (E-134) -- and because four of the seven could not have passed if it had been: `revoke` ended in a call that
//! dereferences kernel globals (the task, scheduler, capability and endpoint tables) on every successful revoke. **The seam is
//! that call.** This crate owns the table and **returns what must be woken** ([`Revoked`], [`RevokedBatch`]); the kernel does the
//! waking, in a wrapper that is the only revoke path it exposes.
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

#![no_std]

use fjell_abi::error::SysError;
use fjell_abi::lease::{LeaseEpoch, LeaseId};
use fjell_abi::task::TaskId;

// `MAX_LEASES` and `MAX_LEASES_PER_OWNER` live in `fjell_abi::lease` (RFC-0.48-001 D1 condition i): services
// derive their own bounds from them -- `neg-test`'s recycling scenario fills the table by the *capacity*, not by a
// second literal that would silently stop exercising reclamation the day the table grew.
pub use fjell_abi::lease::{MAX_LEASES, MAX_LEASES_PER_OWNER};

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
    /// different field.** *(Until 0.49 this sentence said the task table's generation was always `0` because
    /// `TaskTable::remove` had no callers, "and the kernel's `TaskId::new(x, 0)` idiom is correct because of
    /// that". `TaskKill` destroys a task and frees its slot, so a task slot's generation **does** advance, the
    /// idiom is wrong, and `bootstrap-leases`'s premise 6 fails on any production `TaskId::new(_, 0)` outside
    /// the idle task, `init`'s bootstrap and the DMA sentinel -- RFC-0.49-001 D3.)*
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
    ///
    /// **Once the owner is dead, only a `LEASE_REVOKE_ANY` holder can revoke it** (`sys_lease_revoke` admits the owner or
    /// that right); otherwise it lasts until the subject exits or faults. **Task slots are reused since 0.49**, and that is safe
    /// here because `owner` is a full `TaskId` -- index *and* generation -- compared against the caller's whole id
    /// (`owner == current_task_id()`), so a new occupant of a dead owner's slot is a different owner and cannot revoke the
    /// leases of its predecessor's children. *(Until 0.50 this paragraph said slots were never reused, that nothing called
    /// `TaskTable::remove`, and that **no boot scenario demonstrates this skip** because the kernel crate had no host tests. The
    /// first two stopped being true at 0.49; the third at 0.50: `revoke_owned_by_skips_a_boot_lease` demonstrates it.)*
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

/// What a successful revocation did, for the caller to act on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Revoked {
    pub id: LeaseId,
    /// The epoch **after** the revocation.
    pub epoch: LeaseEpoch,
    /// The slot's epoch is at `u32::MAX`: it is retired for good (retire-before-wrap) and its epoch did not advance.
    pub retired: bool,
}

/// The most leases one [`LeaseTable::revoke_owned_by`] call reports: an owner's whole quota ([`MAX_LEASES_PER_OWNER`]).
pub const BATCH: usize = MAX_LEASES_PER_OWNER;

/// The leases one `revoke_owned_by` call revoked.
#[derive(Clone, Copy, Debug)]
pub struct RevokedBatch {
    items: [Revoked; BATCH],
    len: usize,
    /// More owned `Active` leases remain: call again.
    pub more: bool,
}

impl RevokedBatch {
    const fn empty() -> Self {
        RevokedBatch {
            items: [Revoked {
                id: LeaseId(0),
                epoch: LeaseEpoch(0),
                retired: false,
            }; BATCH],
            len: 0,
            more: false,
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = &Revoked> {
        self.items[..self.len].iter()
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
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
    /// Returns the *new* epoch after revocation, **and whether the slot was retired** -- and does nothing else. Waking the tasks
    /// that were blocked in IPC on this lease is the *caller's* job: it touches the task, scheduler, capability and endpoint tables,
    /// which this crate cannot reach, and which is exactly why this function could not be called from a host test while it lived
    /// in the kernel (it ended in a call that dereferences kernel globals, unconditionally, on every successful revoke -- RFC-0.50-001).
    /// The kernel's `LeaseTable` wrapper does the waking and is the only revoke path the kernel exposes.
    ///
    /// After this call all capabilities whose `epoch_at_issue` matches the *old* epoch fail.
    pub fn revoke(&mut self, id: LeaseId) -> Result<Revoked, SysError> {
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
        // retired permanently -- state stays Revoked and the epoch stays at
        // MAX, so every epoch-bound capability keeps failing. (Unreachable in
        // practice: it requires 2^32 revocations of one slot.)
        //
        // History, kept so the claim is not quietly re-added or re-removed: this note used to add
        // "slot reuse bumps the generation, which invalidates old LeaseIds independently" as a second
        // ground. That was FALSE until 0.46 -- no slot was reused and the generation was never
        // incremented -- and it was removed on 2026-10-21 rather than left propping up the argument.
        // Since RFC-0.46-001 it is TRUE: `create` reclaims a revoked slot and bumps the generation, and
        // the Verus model proves it independently of the epoch (LEASE-VERUS-008).
        let retired = match fjell_abi::lease::lease_revoke(slot.epoch) {
            fjell_abi::lease::RevokeOutcome::Advanced(e) => {
                slot.epoch = e;
                false
            }
            // Epoch reached u32::MAX -- retire the slot permanently rather than wrapping (C6, LEASE-VERUS-005). Practically
            // unreachable in any deployment lifetime; reported so the kernel can log it. (Architect follow-up, v0.18 review.)
            fjell_abi::lease::RevokeOutcome::MustRetire => true,
        };
        slot.state = LeaseState::Revoked;
        Ok(Revoked {
            id,
            epoch: LeaseEpoch(slot.epoch),
            retired,
        })
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

    /// Lifecycle revoke: revoke leases owned by `task` (RFC 033 §2.10), up to [`BATCH`] at a time.
    ///
    /// Called when a task exits, faults, or is restarted. Each affected lease is revoked independently; **the caller wakes the
    /// tasks blocked on each one** (see [`LeaseTable::revoke`]) and, if `more` is set, calls again -- the leases already revoked
    /// are no longer `Active`, so the next call finds the rest. A task's quota is [`MAX_LEASES_PER_OWNER`], so one batch is
    /// enough today; `more` exists so that the bound is a stated property and not a silent truncation.
    pub fn revoke_owned_by(&mut self, task: TaskId) -> RevokedBatch {
        let mut batch = RevokedBatch::empty();
        for i in 0..MAX_LEASES {
            // A boot lease belongs to its subject's lifetime, not its owner's (see `LeaseObject::boot`).
            if self.slots[i].state == LeaseState::Active
                && self.slots[i].owner == task
                && !self.slots[i].boot
            {
                if batch.len == BATCH {
                    batch.more = true;
                    break;
                }
                let id = LeaseId::new(i as u16, self.slots[i].generation);
                if let Ok(r) = self.revoke(id) {
                    batch.items[batch.len] = r;
                    batch.len += 1;
                }
            }
        }
        batch
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

// ── RFC 006 + RFC 033: implement LeaseChecker for the lease table ──────

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

    // ── what the move made demonstrable (RFC-0.50-001) ───────────────────────────

    /// The skip `LeaseObject::boot` describes: a task's death must not strip its children of authority. Until 0.50 nothing
    /// could show this -- "removing it turns nothing red" was the RFC-0.48-001 disclosure -- and now something does.
    #[test]
    fn revoke_owned_by_skips_a_boot_lease() {
        let mut lt = LeaseTable::new();
        let task = TaskId::new(4, 0);
        let boot = lt.create_boot(task).unwrap();
        let plain = lt.create(task, 0).unwrap();
        let boot_ep = lt.current_epoch(boot).unwrap();
        let plain_ep = lt.current_epoch(plain).unwrap();
        let b = lt.revoke_owned_by(task);
        assert_eq!(b.len(), 1, "only the plain lease");
        assert_eq!(b.iter().next().unwrap().id, plain);
        assert!(
            lt.check_active(boot, boot_ep).is_ok(),
            "the boot lease belongs to its subject's lifetime, not its owner's"
        );
        assert!(lt.check_active(plain, plain_ep).is_err());
    }

    /// `revoke` reports what the caller must wake; it does not do the waking, and it says when the slot was retired.
    #[test]
    fn revoke_reports_the_new_epoch_and_whether_the_slot_retired() {
        let mut lt = LeaseTable::new();
        let id = lt.create(TaskId::new(1, 0), 0).unwrap();
        let r = lt.revoke(id).unwrap();
        assert_eq!((r.id, r.epoch, r.retired), (id, LeaseEpoch(2), false));
        // an epoch at u32::MAX retires rather than wrapping, and says so
        let id2 = lt.create(TaskId::new(1, 0), 0).unwrap();
        lt.slots[id2.index() as usize].epoch = u32::MAX;
        let r = lt.revoke(id2).unwrap();
        assert!(r.retired);
        assert_eq!(r.epoch, LeaseEpoch(u32::MAX), "the epoch did not wrap");
    }

    /// An owner's whole quota fits one batch, and a larger set is reported as `more`, never silently truncated.
    #[test]
    fn a_batch_holds_an_owners_whole_quota_and_reports_more_when_it_cannot() {
        assert_eq!(BATCH, MAX_LEASES_PER_OWNER);
        let mut lt = LeaseTable::new();
        let task = TaskId::new(2, 0);
        for _ in 0..MAX_LEASES_PER_OWNER {
            lt.create(task, 0).unwrap();
        }
        let b = lt.revoke_owned_by(task);
        assert_eq!((b.len(), b.more), (MAX_LEASES_PER_OWNER, false));
        // the quota stops `create` at BATCH, so build the larger set by hand to show the continuation
        let mut lt = LeaseTable::new();
        for slot in lt.slots.iter_mut().take(BATCH + 3) {
            slot.state = LeaseState::Active;
            slot.epoch = 1;
            slot.owner = task;
        }
        let first = lt.revoke_owned_by(task);
        assert_eq!((first.len(), first.more), (BATCH, true));
        let second = lt.revoke_owned_by(task);
        assert_eq!(
            (second.len(), second.more),
            (3, false),
            "the call continues where the last stopped"
        );
    }
}
