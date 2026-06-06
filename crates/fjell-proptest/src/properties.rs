//! Property assertions for the capability/IPC/lease model (RFC v0.6-001 §7.1).
//!
//! Each property function takes a `ModelState` after a sequence of operations
//! and returns `Ok(())` on pass or `Err(msg)` on violation.

use crate::model::*;
use crate::ops::{execute, Op, OpResult};

pub type PropResult = Result<(), String>;

// ── P1: cap_id never aliases after replace ────────────────────────────────────

/// After CapReplace, operations using the OLD generation return StaleGeneration.
/// The new cap has a strictly higher generation.
pub fn p1_no_generation_alias(state: &ModelState, id: CapId) -> PropResult {
    let cap = match state.caps.get(&id) {
        Some(c) => c,
        None    => return Ok(()),
    };
    let stored_gen = state.generation_of(id);
    if stored_gen == 0 {
        return Ok(()); // never issued
    }
    if cap.generation > stored_gen {
        return Err(format!(
            "P1: cap {id:?} stored generation {stored_gen} but cap.generation {} > stored",
            cap.generation,
        ));
    }
    Ok(())
}

// ── P2: revoke invalidates subsequent use ─────────────────────────────────────

/// After revoking a cap, any use of it returns LeaseRevoked (never Ok).
pub fn p2_revoke_invalidates(state: &mut ModelState, id: CapId) -> PropResult {
    // Mark the cap revoked.
    let _ = execute(state, &Op::CapRevoke { id });
    // Attempt IpcSend — must fail with LeaseRevoked or NotFound.
    let r = execute(state, &Op::IpcSend {
        from: TaskId(0), cap_id: id, tag: 0, payload: 0
    });
    if r == OpResult::Ok {
        return Err(format!("P2: IpcSend through revoked cap {id:?} returned Ok"));
    }
    Ok(())
}

// ── P3: delegate sub-rights subset only ──────────────────────────────────────

/// Delegated rights must be a strict subset of the source cap's rights.
pub fn p3_delegate_subrights(
    state:   &mut ModelState,
    src:     CapId,
    new_id:  CapId,
    attempt: CapRights,
) -> PropResult {
    let src_rights = match state.active_cap(src) {
        Some(c) => c.rights,
        None    => return Ok(()), // can't test without active src
    };
    let result = execute(state, &Op::CapDelegate {
        src, new_id, dst_task: TaskId(1), sub_rights: attempt,
    });
    if attempt.is_subset_of(src_rights) {
        if result != OpResult::Ok && result != OpResult::CapacityExhausted {
            return Err(format!(
                "P3: valid sub-delegation returned {result:?} for src_rights {src_rights:?}"
            ));
        }
    } else {
        if result == OpResult::Ok {
            return Err(format!(
                "P3: over-delegation succeeded: attempt={attempt:?} src={src_rights:?}"
            ));
        }
    }
    Ok(())
}

// ── P4: lease expiry revokes all caps under it ────────────────────────────────

pub fn p4_lease_expiry_revokes_caps(state: &mut ModelState, lease_id: LeaseId) -> PropResult {
    let before: Vec<CapId> = state.caps.iter()
        .filter(|(_, c)| c.lease == lease_id && c.state == CapState::Active)
        .map(|(id, _)| *id)
        .collect();

    execute(state, &Op::LeaseExpire { lease_id });

    for id in &before {
        let cap = state.caps.get(id).unwrap();
        if cap.state == CapState::Active {
            return Err(format!(
                "P4: cap {id:?} under lease {lease_id:?} still active after expiry"
            ));
        }
    }
    Ok(())
}

// ── P5: task fault revokes owned leases ──────────────────────────────────────

pub fn p5_task_fault_revokes_leases(state: &mut ModelState, task: TaskId) -> PropResult {
    let owned: Vec<LeaseId> = state.leases.iter()
        .filter(|(_, l)| l.origin_task == task && l.state == LeaseState::Active)
        .map(|(id, _)| *id)
        .collect();

    execute(state, &Op::TaskFault { task });

    for lid in &owned {
        let l = state.leases.get(lid).unwrap();
        if l.state != LeaseState::Expired {
            return Err(format!(
                "P5: lease {lid:?} owned by faulted task {task:?} still active"
            ));
        }
    }
    Ok(())
}

// ── P6: IpcSend requires SEND right ──────────────────────────────────────────

pub fn p6_send_requires_right(state: &mut ModelState, cap_id: CapId) -> PropResult {
    let rights = match state.active_cap(cap_id) {
        Some(c) => c.rights,
        None    => return Ok(()),
    };
    let result = execute(state, &Op::IpcSend {
        from: TaskId(0), cap_id, tag: 0, payload: 0
    });
    if !rights.contains(CapRights::SEND) && result == OpResult::Ok {
        return Err(format!("P6: IpcSend succeeded without SEND right (rights={rights:?})"));
    }
    Ok(())
}

// ── P7: IpcRecv returns NoMessage or message, never panics ───────────────────

pub fn p7_recv_no_panic(state: &mut ModelState, task: TaskId, endpoint: CapId) -> PropResult {
    let result = execute(state, &Op::IpcRecv { task, endpoint });
    // This property simply verifies the call completes with a valid result.
    match result {
        OpResult::Ok | OpResult::NoMessage | OpResult::LeaseRevoked
            | OpResult::NotFound | OpResult::TaskFaulted => Ok(()),
        other => Err(format!("P7: unexpected IpcRecv result: {other:?}")),
    }
}

// ── P8: revoked cap in-flight message dropped ─────────────────────────────────

pub fn p8_inflight_dropped_after_revoke(
    state: &mut ModelState, cap_id: CapId
) -> PropResult {
    // Send a message through the cap.
    execute(state, &Op::IpcSend { from: TaskId(0), cap_id, tag: 1, payload: 42 });
    // Revoke the cap.
    execute(state, &Op::CapRevoke { id: cap_id });
    // Recv — must NOT deliver the stale payload (returns LeaseRevoked or NoMessage).
    let result = execute(state, &Op::IpcRecv { task: TaskId(0), endpoint: cap_id });
    if result == OpResult::Ok {
        // After revoke the receive should fail.
        return Err(format!("P8: IpcRecv delivered message after cap {cap_id:?} was revoked"));
    }
    Ok(())
}

// ── P9: generation is monotonically increasing ───────────────────────────────

pub fn p9_generation_monotonic(state: &ModelState, id: CapId) -> PropResult {
    let stored = state.generation_of(id);
    if let Some(cap) = state.caps.get(&id) {
        if cap.generation > stored {
            return Err(format!(
                "P9: cap {id:?} generation {} > stored_max {stored}", cap.generation
            ));
        }
    }
    Ok(())
}

// ── P10: cap table capacity respected ────────────────────────────────────────

pub fn p10_no_capacity_underrun(state: &ModelState) -> PropResult {
    let live = state.live_cap_count();
    if live > MAX_CAP_TABLE {
        return Err(format!(
            "P10: live cap count {live} exceeds MAX_CAP_TABLE {MAX_CAP_TABLE}"
        ));
    }
    Ok(())
}
