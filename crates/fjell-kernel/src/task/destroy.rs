//! RFC-0.49-001 R2: **destroy a task** -- stop it, take back everything the kernel holds on its behalf, free its slot.
//!
//! Until 0.49 a task could end only by `sys_exit` (itself) or a fault, and **the slot it ended in was never returned**:
//! `TaskTable::remove` had no caller, so a boot could spawn at most `MAX_TASKS` tasks for its whole uptime, and a task
//! that neither exited nor faulted could not be stopped by anyone. `TaskKill` is the one verb that closes both, with one
//! meaning: **afterwards the id names nothing.**
//!
//! * a **live** task (`Created`, `Runnable`, `Blocked`): the death path runs (it never did for a task that did not end
//!   itself), then everything below;
//! * a task that has **already exited or faulted**: its death path ran when it died, so only the slot is released. Its
//!   status stays readable by `sys_task_status` *until* a `TASK_KILL` holder destroys it -- a caller that wants the verdict
//!   reads it first (RFC-0.49-001 D1, ruled (B)).
//!
//! **What it frees.** User pages (matched on the whole `TaskId`, so a previous occupant's frames are never confused with
//! the current one's), the kernel stack, the CSpace (every slot emptied, every slot generation bumped), the reply edges
//! naming the slot (as server *and* as caller), the endpoint queue entries, the audit-ring waiter, the interrupt lines, the
//! ready-queue entry, DMA regions and leases (through the death path), and the slot itself (its generation advances).
//!
//! **What it does not free: the dead task's page-table frames** (about 6 per task, derived) -- a walker over a live
//! structure whose upper half is shared with the kernel, deferred with its reason in RFC-0.49-001 D4. The leak is a stated
//! bound, not an accident: `destroy` reports it so a test can pin it.

use crate::task::TaskId;
use crate::task::tcb::TaskState;
use fjell_abi::error::SysError;

/// Destroy `target`. `caller` is the task asking; **it may not destroy itself** (`sys_exit` is the verb for that) and
/// **nothing may destroy the idle task** (slot 0, which the scheduler falls back to).
/// Returns the number of frames returned to the allocator (user pages plus the kernel stack).
pub fn destroy_task(target: TaskId, caller: TaskId) -> Result<u64, SysError> {
    fjell_cap::task_kill::kill_target_permitted(target, caller)?;
    destroy_inner(target, Some(caller))
}

/// Undo a spawn that failed **after** its slot was taken (RFC-0.49-001 R2): the task never ran, but it holds a slot, a
/// CSpace, frames and a boot lease. Same release as [`destroy_task`], with no caller to check.
pub(crate) fn rollback_spawn(target: TaskId) {
    let _ = destroy_inner(target, None);
}

fn destroy_inner(target: TaskId, caller: Option<TaskId>) -> Result<u64, SysError> {
    // SAFETY: category=kernel-global-mutable single-hart; no concurrent access.
    let (table, sched, ct, et) = unsafe { crate::get_kernel_state() };
    // Copy out what is needed and drop the borrow: the revocation below reaches back into the task table.
    let (was_live, boot_lease, kstack_top, image_id, frames_taken, was_clean_exit) =
        match table.get(target) {
            None => return Err(SysError::InvalidCap),
            Some(t) => (
                !matches!(t.state, TaskState::Exited(_) | TaskState::Faulted(_)),
                t.boot_lease,
                t.kernel_stack_top,
                t.image_id,
                t.frames_taken,
                matches!(t.state, TaskState::Exited(0)),
            ),
        };
    let index = target.index;
    // SAFETY: category=kernel-global-mutable lease table single-threaded.
    let lt = unsafe { crate::get_lease_table() };

    let ep_ids: [u32; crate::cap::table::MAX_ENDPOINTS] = {
        let mut arr = [0u32; crate::cap::table::MAX_ENDPOINTS];
        let mut n = 0usize;
        for id in et.iter_allocated_ids() {
            if n < arr.len() {
                arr[n] = id;
                n += 1;
            }
        }
        arr
    };
    // Counted BEFORE anything is revoked: a receiver whose endpoint capability is lease-bound is removed from its queue by the
    // lease revocation below (`cancel_by_lease`), so counting afterwards would report 0 for a task that was blocked. The
    // *explicit* purge further down is what catches a waiter no lease reaches -- an unbound capability.
    let mut was_queued_in = 0usize;
    for &ep_id in ep_ids.iter() {
        if et.get_mut(ep_id).is_some_and(|ep| ep.mentions(index)) {
            was_queued_in += 1;
        }
    }

    // 1. The death path, for a task that did not run it itself.
    if was_live {
        crate::trap::syscall::debug_leave(index as usize);
        crate::dma_table().release_task(target);
        crate::mmio_mappings().release_task(target);
        lt.revoke_owned_by(target);
        if let Some(bl) = boot_lease {
            let _ = lt.revoke(bl);
        }
        if let Some(t) = table.get_mut(target) {
            t.boot_lease = None;
            t.state = TaskState::Exited(KILLED_EXIT_CODE);
        }
    }

    // 2. Everything that names the slot by index (RFC-0.49-001 D5). After this, no queue, edge, waiter or binding refers
    //    to `index`, which is what makes an index in a live structure always name the *current* occupant.
    for &ep_id in ep_ids.iter() {
        if let Some(ep) = et.get_mut(ep_id) {
            ep.cancel(index);
        }
    }
    // Callers blocked waiting for THIS task's reply would wait for ever: wake them with `Canceled`.
    let (waiting_callers, n_waiting) = ct.purge_replies_for_task(index);
    for &caller_idx in waiting_callers.iter().take(n_waiting) {
        crate::cap::syscall::wake_with_error(table, sched, caller_idx, SysError::Canceled);
    }
    crate::audit::ring::AUDIT.forget(target);
    crate::irq_table().release_task(target);
    sched.forget(target);

    // **Verified, not assumed:** after the purge no endpoint queue and no reply edge may still name the slot. A residue here is
    // the failure this whole step exists to prevent, so it is looked for and said (the scenario's profile requires the
    // all-clear wording; the residue wording would not match it).
    let mut residual = 0usize;
    for &ep_id in ep_ids.iter() {
        if et.get_mut(ep_id).is_some_and(|ep| ep.mentions(index)) {
            residual += 1;
        }
    }
    if ct.peek_reply(index as usize).is_some() {
        residual += 1;
    }

    // 3. The CSpace: empty, every slot generation bumped (so a stale handle names nothing, and the next occupant's grants
    //    are not refused by `install_raw` against the predecessor's capabilities).
    if let Some(cs) = ct.cspace_mut(index as usize) {
        cs.reset();
    }

    // 4. Frames.
    // SAFETY: category=kernel-global-mutable frame allocator, single hart.
    let fa = unsafe { &mut *crate::fa_static_ptr() };
    let mut frames_freed = fa.free_user_frames_of(target);
    // The kernel stack carries no task tag, so it is freed by address -- and only if the allocator agrees it is one.
    if kstack_top >= 4096 {
        if let Ok(f) = crate::mm::frame_alloc::PhysFrame::from_pa(kstack_top - 4096) {
            if fa.owner_of(f) == Some(crate::mm::frame_alloc::FrameOwner::KernelStack)
                && fa.free_frame(f).is_ok()
            {
                frames_freed += 1;
            }
        }
    }

    // 5. The slot. `remove` bumps its generation, so every id the task ever had now names nothing.
    let _ = table.remove(target);
    // The kernel's milestone markers ask "has this image exited cleanly", and must still be able to say yes once the corpse
    // that used to answer it is gone (RFC-0.49-001 D1). Said, not assumed.
    let latch_note = if was_clean_exit {
        if crate::trap::dispatch::exited_ok_by_image(table, image_id) {
            "; exited cleanly: the milestone latch for its image is still true after the corpse was destroyed"
        } else {
            "; exited cleanly: LOST -- the milestone latch for its image is false once the corpse is gone"
        }
    } else {
        ""
    };
    match caller {
        Some(c) => crate::kprintln!(
            "[task#{} image={}]: destroyed by task#{} ({} of {} frames freed, {} page-table frame(s) retained; was blocked in {} endpoint queue(s); {}{})",
            index,
            image_id.0,
            c.index,
            frames_freed,
            frames_taken,
            (frames_taken as u64).saturating_sub(frames_freed),
            was_queued_in,
            if residual == 0 {
                "no queue or reply edge names the slot"
            } else {
                "RESIDUE: a queue or reply edge still names the slot"
            },
            latch_note
        ),
        None => crate::kprintln!(
            "[task#{} image={}]: spawn rolled back ({} frames freed)",
            index,
            image_id.0,
            frames_freed
        ),
    }
    Ok(frames_freed)
}

/// The exit code a live task is left in for the instant between "stopped" and "slot removed". Nothing observes it.
const KILLED_EXIT_CODE: i32 = -9;
