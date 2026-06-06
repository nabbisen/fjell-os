//! Operations on `ModelState` (RFC v0.6-001 §6.1).

use crate::model::*;

// ── Operation type ────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum Op {
    CapRegister  { id: CapId, kind: CapKind, rights: CapRights, lease: LeaseId },
    CapDelegate  { src: CapId, new_id: CapId, dst_task: TaskId, sub_rights: CapRights },
    CapRevoke    { id: CapId },
    CapReplace   { id: CapId, new_kind: CapKind, new_rights: CapRights },
    IpcSend      { from: TaskId, cap_id: CapId, tag: u16, payload: u64 },
    IpcRecv      { task: TaskId, endpoint: CapId },
    LeaseExpire  { lease_id: LeaseId },
    LeaseRenew   { lease_id: LeaseId },
    TaskFault    { task: TaskId },
    Tick,
}

// ── Result type ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OpResult {
    Ok,
    LeaseRevoked,
    StaleGeneration,
    RightsDenied,
    NoMessage,
    NotFound,
    CapacityExhausted,
    TaskFaulted,
}

// ── Execution ─────────────────────────────────────────────────────────────────

pub fn execute(state: &mut ModelState, op: &Op) -> OpResult {
    match op {
        Op::CapRegister { id, kind, rights, lease } => {
            // Reject if the table is full.
            if state.live_cap_count() >= MAX_CAP_TABLE {
                return OpResult::CapacityExhausted;
            }
            // Reject if the lease is not active.
            if state.leases.get(lease).map_or(true, |l| l.state != LeaseState::Active) {
                return OpResult::LeaseRevoked;
            }
            let origin = TaskId(0);
            let generation = state.next_generation(*id);
            state.caps.insert(*id, ModelCap {
                kind: *kind, rights: *rights,
                origin_task: origin, lease: *lease,
                generation, state: CapState::Active,
            });
            OpResult::Ok
        }

        Op::CapDelegate { src, new_id, dst_task, sub_rights } => {
            // src must be active and owned by a running task.
            let src_cap = match state.active_cap(*src) {
                Some(c) => c.clone(),
                None    => return OpResult::LeaseRevoked,
            };
            // Delegate rights must be subset of source.
            if !sub_rights.is_subset_of(src_cap.rights) {
                return OpResult::RightsDenied;
            }
            if state.live_cap_count() >= MAX_CAP_TABLE {
                return OpResult::CapacityExhausted;
            }
            let generation = state.next_generation(*new_id);
            state.caps.insert(*new_id, ModelCap {
                kind:        src_cap.kind,
                rights:      *sub_rights,
                origin_task: *dst_task,
                lease:       src_cap.lease,
                generation,
                state:       CapState::Active,
            });
            OpResult::Ok
        }

        Op::CapRevoke { id } => {
            if let Some(cap) = state.caps.get_mut(id) {
                if cap.state == CapState::Active {
                    cap.state = CapState::Revoked;
                    return OpResult::Ok;
                }
            }
            OpResult::NotFound
        }

        Op::CapReplace { id, new_kind, new_rights } => {
            if let Some(cap) = state.caps.get_mut(id) {
                let old_lease = cap.lease;
                let old_origin = cap.origin_task;
                cap.state    = CapState::Replaced;
                let generation = state.next_generation(*id);
                // Re-insert as new cap.
                let new_cap = ModelCap {
                    kind:        *new_kind,
                    rights:      *new_rights,
                    origin_task: old_origin,
                    lease:       old_lease,
                    generation,
                    state:       CapState::Active,
                };
                state.caps.insert(*id, new_cap);
                return OpResult::Ok;
            }
            OpResult::NotFound
        }

        Op::IpcSend { from, cap_id, tag, payload } => {
            // Task must be running.
            if state.tasks.get(from).map_or(true, |t| t.state != TaskState::Running) {
                return OpResult::TaskFaulted;
            }
            // Cap must be active.
            let cap = match state.active_cap(*cap_id) {
                Some(c) => c.clone(),
                None    => return OpResult::LeaseRevoked,
            };
            // Must have SEND right.
            if !cap.rights.contains(CapRights::SEND) {
                return OpResult::RightsDenied;
            }
            // Deliver to all recv-capable tasks (model: deliver to endpoint mailboxes).
            // For simplicity: deliver to the task that owns the endpoint's origin.
            if let Some(task) = state.tasks.get_mut(&cap.origin_task) {
                task.mailbox.push((*cap_id, *tag, *payload));
            }
            OpResult::Ok
        }

        Op::IpcRecv { task, endpoint } => {
            // Cap must be active.
            if state.active_cap(*endpoint).is_none() {
                return OpResult::LeaseRevoked;
            }
            if let Some(t) = state.tasks.get_mut(task) {
                if t.mailbox.is_empty() {
                    return OpResult::NoMessage;
                }
                t.mailbox.remove(0);
                return OpResult::Ok;
            }
            OpResult::NotFound
        }

        Op::LeaseExpire { lease_id } => {
            if let Some(lease) = state.leases.get_mut(lease_id) {
                lease.state = LeaseState::Expired;
            }
            // Revoke all caps under this lease.
            for cap in state.caps.values_mut() {
                if cap.lease == *lease_id && cap.state == CapState::Active {
                    cap.state = CapState::Revoked;
                }
            }
            OpResult::Ok
        }

        Op::LeaseRenew { lease_id } => {
            if let Some(lease) = state.leases.get_mut(lease_id) {
                if lease.state == LeaseState::Expired {
                    lease.state = LeaseState::Active;
                }
                return OpResult::Ok;
            }
            OpResult::NotFound
        }

        Op::TaskFault { task } => {
            if let Some(t) = state.tasks.get_mut(task) {
                t.state = TaskState::Faulted;
            }
            // Expire all leases originated by this task.
            let faulted_leases: Vec<LeaseId> = state.leases.iter()
                .filter(|(_, l)| l.origin_task == *task)
                .map(|(id, _)| *id)
                .collect();
            for lid in faulted_leases {
                execute(state, &Op::LeaseExpire { lease_id: lid });
            }
            OpResult::Ok
        }

        Op::Tick => {
            state.now += 1;
            OpResult::Ok
        }
    }
}
