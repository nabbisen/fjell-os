//! Capability and IPC syscall handlers for M3.

#![allow(dead_code)]

use fjell_abi::error::SysError;
use fjell_cap::{CapHandle, CapKind, CapRights};
use fjell_ipc::endpoint::{PendingMessage, SendResult, RecvResult};
use fjell_ipc::message::{MessageTag, IPC_WORDS};
use crate::{
    audit::ring::{AuditKindInternal, AUDIT},
    task::{
        scheduler::{Scheduler, PRIORITY_USER},
        tcb::{BlockReason, TaskState, TaskTable, TrapFrame, REG_A0, REG_A1, REG_A2, REG_A3},
        TaskId,
    },
};
use super::table::{CapTable, EndpointTable};

// ── ABI helpers ───────────────────────────────────────────────────────────────

fn ok(tf: &mut TrapFrame)              { tf.gpr[REG_A0] = SysError::Ok as isize as usize; }
fn err(tf: &mut TrapFrame, e: SysError){ tf.gpr[REG_A0] = e as isize as usize; }

// ── Capability syscalls ───────────────────────────────────────────────────────

pub fn sys_cap_copy(tf: &mut TrapFrame, tidx: usize, ct: &mut CapTable) {
    let src = CapHandle(tf.gpr[10] as u32);
    let dst  = tf.gpr[11];
    // RFC 049: require COPY right + lease check on source cap.
    {
        // SAFETY: capability handle is validated by require_cap before this call.
        let lt = unsafe { crate::get_lease_table() };
        let cs = match ct.cspace(tidx) { Some(c) => c, None => { err(tf, SysError::InternalError); return; }};
        let cap = match cs.get(src) { Ok(c) => c, Err(e) => { err(tf, e); return; }};
        if let Err(e) = cap.check_lease(lt) { err(tf, e.into()); return; }
        if !cap.rights.contains(CapRights::COPY) { err(tf, SysError::PermissionDenied); return; }
    }
    let cs = match ct.cspace_mut(tidx) { Some(c) => c, None => { err(tf, SysError::InternalError); return; } };
    match cs.copy(src, dst) {
        Ok(h)  => { ok(tf); tf.gpr[REG_A1] = h.0 as usize; }
        Err(e) => err(tf, e),
    }
    AUDIT.lock_free_append(AuditKindInternal::CapCopy, src.0 as usize, dst, 0);
}

pub fn sys_cap_mint(tf: &mut TrapFrame, tidx: usize, ct: &mut CapTable) {
    let src    = CapHandle(tf.gpr[10] as u32);
    let dst    = tf.gpr[11];
    let rights = CapRights(tf.gpr[12] as u64);  // v0.2: CapRights is u64
    // RFC 049: require MINT right + lease check on source cap.
    {
        // SAFETY: capability handle is validated by require_cap before this call.
        let lt = unsafe { crate::get_lease_table() };
        let cs = match ct.cspace(tidx) { Some(c) => c, None => { err(tf, SysError::InternalError); return; }};
        let cap = match cs.get(src) { Ok(c) => c, Err(e) => { err(tf, e); return; }};
        if let Err(e) = cap.check_lease(lt) { err(tf, e.into()); return; }
        if !cap.rights.contains(CapRights::MINT) { err(tf, SysError::PermissionDenied); return; }
    }
    let badge  = tf.gpr[13] as u64;
    let cs = match ct.cspace_mut(tidx) { Some(c) => c, None => { err(tf, SysError::InternalError); return; } };
    match cs.mint(src, dst, rights, badge) {
        Ok(h)  => { ok(tf); tf.gpr[REG_A1] = h.0 as usize; }
        Err(e) => err(tf, e),
    }
    AUDIT.lock_free_append(AuditKindInternal::CapMint, src.0 as usize, dst, 0);
}

pub fn sys_cap_delete(tf: &mut TrapFrame, tidx: usize, ct: &mut CapTable) {
    let h = CapHandle(tf.gpr[10] as u32);
    let cs = match ct.cspace_mut(tidx) { Some(c) => c, None => { err(tf, SysError::InternalError); return; } };
    match cs.delete(h) { Ok(()) => ok(tf), Err(e) => err(tf, e) }
    AUDIT.lock_free_append(AuditKindInternal::CapDelete, h.0 as usize, 0, 0);
}

pub fn sys_cap_revoke(tf: &mut TrapFrame, tidx: usize, ct: &mut CapTable) {
    let h = CapHandle(tf.gpr[10] as u32);
    // RFC 049: require REVOKE right + lease check on source cap.
    {
        // SAFETY: capability handle is validated by require_cap before this call.
        let lt = unsafe { crate::get_lease_table() };
        let cs = match ct.cspace(tidx) { Some(c) => c, None => { err(tf, SysError::InternalError); return; }};
        let cap = match cs.get(h) { Ok(c) => c, Err(e) => { err(tf, e); return; }};
        if let Err(e) = cap.check_lease(lt) { err(tf, e.into()); return; }
        if !cap.rights.contains(CapRights::REVOKE) { err(tf, SysError::PermissionDenied); return; }
    }
    let cs = match ct.cspace_mut(tidx) { Some(c) => c, None => { err(tf, SysError::InternalError); return; } };
    match cs.revoke(h) { Ok(()) => ok(tf), Err(e) => err(tf, e) }
    AUDIT.lock_free_append(AuditKindInternal::CapRevoke, h.0 as usize, 0, 0);
}

pub fn sys_cap_inspect(tf: &mut TrapFrame, tidx: usize, ct: &CapTable) {
    let h  = CapHandle(tf.gpr[10] as u32);
    // SAFETY: capability handle is validated by require_cap before this call.
    let lt = unsafe { crate::get_lease_table() };
    let cs = match ct.cspace(tidx) { Some(c) => c, None => { err(tf, SysError::InternalError); return; } };
    // RFC 049: require INSPECT right + lease check before exposing cap metadata.
    match cs.get(h) {
        Ok(cap) => {
            if let Err(e) = cap.check_lease(lt) { err(tf, e.into()); return; }
            if !cap.rights.contains(CapRights::INSPECT) { err(tf, SysError::PermissionDenied); return; }
        }
        Err(e)  => { err(tf, e); return; }
    }
    match cs.inspect(h) {
        Ok((kind, rights, badge)) => {
            ok(tf);
            tf.gpr[REG_A1] = kind as usize;
            tf.gpr[12]     = rights.0 as usize;
            tf.gpr[13]     = badge as usize;
        }
        Err(e) => err(tf, e),
    }
}


/// `sys_cap_install(a0=install_cap, a1=target_tid_raw, a2=cap_kind, a3=object_id)` — RFC 056.
///
/// Installs a capability into `target_tid`'s CSpace.  Requires a `CapInstall` cap
/// with `CAP_INSTALL` right.  Returns `Ok(cap_handle)` on success.
///
/// Installed cap has `ALL` rights, `ObjectScope::Any`, and no lease — the
/// caller (cap-broker) may refine rights via a subsequent `cap_mint` call.
pub fn sys_cap_install(tf: &mut TrapFrame, tidx: usize, ct: &mut CapTable) {
    use fjell_cap::{CapKind, CapRights, CapState, rights::ObjectScope, slot::Capability};
    use crate::task::TaskId;

    let install_cap = fjell_cap::handle::CapHandle(tf.gpr[REG_A0] as u32);
    let target_raw  = tf.gpr[REG_A1] as u32;
    let cap_kind_n  = tf.gpr[REG_A2] as u8;
    let object_id   = tf.gpr[REG_A3] as u32;

    // 1. Validate the CapInstall authority.
    if let Err(e) = crate::trap::syscall::require_cap_on_ct(ct, tidx, install_cap,
                                        CapKind::CapInstall, CapRights::CAP_INSTALL, None) {
        err(tf, e); return;
    }

    // 2. Parse target task id.
    let target_idx = (target_raw & 0xFFFF) as u16;
    let target_gen  = (target_raw >> 16) as u16;
    let _target_id  = TaskId::new(target_idx, target_gen);  // scope-check deferred (V02-A-005)

    // 3. Map cap_kind discriminant to CapKind.
    // RFC-v0.7.4-003: unknown kind → InvalidArg (was silently coerced to Endpoint).
    let kind = match CapKind::from_u8(cap_kind_n) {
        Some(k) => k,
        None    => { err(tf, SysError::InvalidArg); return; }
    };

    // 4. Install into target CSpace.
    // RFC-v0.7.4-003: use ALL_NON_META; CAP_INSTALL itself stays with cap-broker.
    // cap-broker may further narrow rights via cap_mint after installation.
    #[allow(deprecated)]  // intentional: CapRights::ALL is the old name we still accept
    let cap = Capability {
        kind, object_id,
        rights: CapRights::ALL_NON_META,
        badge: 0,
        scope: ObjectScope::Any,
        state: CapState::Active,
        parent: None,
        lease: None,
    };
    let target_tidx = target_idx as usize;
    let ts = match ct.cspace_mut(target_tidx) {
        Some(c) => c,
        None    => { err(tf, SysError::InvalidCap); return; }
    };
    match ts.install_any(cap) {
        Ok(h)  => { ok(tf); tf.gpr[REG_A1] = h.0 as usize; }
        Err(e) => { err(tf, e); }
    }
    AUDIT.lock_free_append(AuditKindInternal::CapMint,
                           target_idx as usize, object_id as usize, 0);
}

// ── IPC syscalls ──────────────────────────────────────────────────────────────

/// Build a PendingMessage from the trap frame.
/// a0 = ep_handle, a1 = packed tag, a2..a5 = words 0..3
fn build_msg(
    tf:   &TrapFrame,
    tidx: usize,
    ct:   &CapTable,
    is_call: bool,
) -> Result<(u32, PendingMessage), SysError> {
    let ep_h  = CapHandle(tf.gpr[10] as u32);
    let raw   = tf.gpr[11] as u64;
    let tag   = MessageTag {
        label: (raw & 0xFFFF) as u16,
        words: ((raw >> 16) & 0xFF) as u8,
        caps:  ((raw >> 24) & 0xFF) as u8,
        flags: 0, _pad: 0,
    };
    if !tag.is_valid() { return Err(SysError::MsgTooLong); }

    let cs  = ct.cspace(tidx).ok_or(SysError::InternalError)?;
    let cap = cs.get(ep_h)?;
    if cap.kind != CapKind::Endpoint { return Err(SysError::WrongType); }

    let mut words = [0u64; IPC_WORDS];
    for i in 0..(tag.words as usize).min(4) { words[i] = tf.gpr[12 + i] as u64; }

    // RFC 055: look up sender's image_id for kernel-attested identity.
    let sender_image_id = {
        // SAFETY: capability handle is validated by require_cap before this call.
        let (table, _, _, _) = unsafe { crate::get_kernel_state() };
        let sender_id = crate::task::TaskId::new(tidx as u16, 0);
        table.get(sender_id).map(|t| t.image_id.0).unwrap_or(0xFFFF)
    };

    Ok((cap.object_id, PendingMessage {
        tag,
        sender_tid:      tidx as u16,
        sender_image_id,
        sender_badge:    cap.badge,
        words,
        cap_present: false, cap_kind: 0, cap_obj_id: 0, cap_rights: 0,
        is_call,
        lease: cap.lease,
    }))
}

fn check_right(tf: &TrapFrame, tidx: usize, ct: &CapTable, right: CapRights) -> Result<(), SysError> {
    let ep_h = CapHandle(tf.gpr[10] as u32);
    let cs   = ct.cspace(tidx).ok_or(SysError::InternalError)?;
    let cap  = cs.get(ep_h)?;
    if !cap.rights.contains(right) { return Err(SysError::PermissionDenied); }
    // RFC 015: validate lease binding — revoked caps must not be used for IPC.
    // SAFETY: capability handle is validated by require_cap before this call.
    let lt = unsafe { crate::get_lease_table() };
    cap.check_lease(lt).map_err(SysError::from)?;
    Ok(())
}

/// `sys_cap_drop(a0=cap_handle) -> a0=status`
///
/// RFC 032 (v0.2.0): Explicitly drop a capability slot so it can be reused.
/// Unlike `cap_delete`, succeeds even when the capability's lease is revoked.
pub fn sys_cap_drop(tf: &mut TrapFrame, tidx: usize, ct: &mut CapTable) {
    let h  = CapHandle(tf.gpr[10] as u32);
    let cs = match ct.cspace_mut(tidx) {
        Some(c) => c,
        None    => { err(tf, SysError::InternalError); return; }
    };
    match fjell_cap::enforcement::cap_drop(cs, h) {
        Ok(()) => {
            ok(tf);
            AUDIT.lock_free_append(AuditKindInternal::CapDrop, h.0 as usize, 0, 0);
        }
        Err(e) => err(tf, e.into()),
    }
}

/// `sys_cap_bind_lease(a0=cap_handle, a1=lease_id) -> a0=status`
///
/// RFC 042: bind a lease to an existing capability so that `require_cap`
/// step 7 verifies the lease is still active on every use.
///
/// Requires: caller holds `CapKind::LeaseAdmin` with `CapRights::LEASE_CREATE`.
/// This ensures only privileged services (init, service-manager) can create
/// lease-bound caps, preventing unprivileged escalation.
pub fn sys_cap_bind_lease(tf: &mut TrapFrame, tidx: usize, ct: &mut CapTable) {
    use fjell_abi::lease::LeaseId;

    let cap_h    = CapHandle(tf.gpr[REG_A0] as u32);
    let lease_id = LeaseId(tf.gpr[REG_A1] as u32);

    // RFC-v0.7.4-003 (closes C-RB-05): enforce documented LeaseAdmin authority.
    // The caller MUST hold CapKind::LeaseAdmin with CapRights::LEASE_CREATE.
    // Previously this check was deferred (V02-A-001); now fully enforced.
    //
    // We scan the caller's CSpace for any LeaseAdmin cap with LEASE_CREATE.
    // This is O(CSPACE_SLOTS) but bind_lease is rare (setup-time only).
    {
        // SAFETY: category=kernel-global-mutable
        //   single-hart kernel; get_kernel_state returns globally unique ptrs.
        let (_, _, ct_ref, _) = unsafe { crate::get_kernel_state() };
        // SAFETY: category=kernel-global-mutable  lease table single-threaded.
        let lt_ref = unsafe { crate::get_lease_table() };
        let cs = match ct_ref.cspace(tidx) {
            Some(c) => c,
            None    => { err(tf, SysError::InternalError); return; }
        };
        let found = cs.iter_occupied().any(|cap| {
            cap.kind == CapKind::LeaseAdmin
                && cap.rights.contains(CapRights::LEASE_CREATE)
                && cap.check_lease(lt_ref).is_ok()
        });
        if !found {
            err(tf, SysError::PermissionDenied);
            AUDIT.lock_free_append(AuditKindInternal::CapDenied, tidx, 0, 0);
            return;
        }
    }

    // 2. Get the current epoch for the lease (must be Active).
    // SAFETY: category=kernel-global-mutable  lease table single-threaded.
    let lt = unsafe { crate::get_lease_table() };
    let epoch = match lt.current_epoch(lease_id) {
        Ok(e)  => e,
        Err(e) => { err(tf, e); return; }
    };

    // 3. Bind the lease to the cap in the caller's CSpace.
    // Fixed in v0.2.9 (RB-03): use slot_by_handle_mut for generation-validated
    // lookup.  v0.2.8 used `cap_h.0 as usize` which conflated handle with slot
    // index — fine while generations stayed at 0, but incorrect by design.
    let cs = match ct.cspace_mut(tidx) {
        Some(c) => c,
        None    => { err(tf, SysError::InternalError); return; }
    };
    let binding = fjell_cap::slot::LeaseBinding { lease_id, epoch_at_issue: epoch };
    match cs.slot_by_handle_mut(cap_h) {
        Ok(slot) => {
            let idx = cap_h.slot() as usize;
            match &mut slot.cap {
                Some(cap) => {
                    cap.lease = Some(binding);
                    ok(tf);
                    AUDIT.lock_free_append(AuditKindInternal::CapMint, idx, lease_id.0 as usize, 0);
                }
                None => err(tf, SysError::InvalidCap),
            }
        }
        Err(_) => { err(tf, SysError::InvalidCap); }
    }
}

/// Deliver a PendingMessage into the current task's TrapFrame (for recv/call).
fn deliver(tf: &mut TrapFrame, msg: &PendingMessage) {
    ok(tf);
    let packed = (msg.tag.label as usize)
        | ((msg.tag.words as usize) << 16)
        | ((msg.tag.caps  as usize) << 24);
    tf.gpr[REG_A1] = packed;
    tf.gpr[12]     = msg.sender_badge as usize;
    for i in 0..(msg.tag.words as usize).min(4) { tf.gpr[13 + i] = msg.words[i] as usize; }
    // RFC 055: a6 = (sender_tid | sender_image_id << 16) — kernel-attested identity.
    tf.gpr[16] = (msg.sender_tid as usize) | ((msg.sender_image_id as usize) << 16);
}

pub fn sys_ipc_send(
    tf: &mut TrapFrame, tidx: usize,
    ct: &mut CapTable, et: &mut EndpointTable,
    tasks: &mut TaskTable, sched: &mut Scheduler,
    cur_id: TaskId,
) {
    // RFC-v0.7.4-003 / C-M-07: debug UART writes removed from production IPC path.
    // IPC events are recorded in the audit ring via AuditKindInternal::IpcSend.
    if let Err(e) = check_right(tf, tidx, ct, CapRights::SEND) {
        err(tf, e); return;
    }
    let (ep_id, msg) = match build_msg(tf, tidx, ct, false) { Ok(x) => x, Err(e) => { err(tf, e); return; } };
    let ep = match et.get_mut(ep_id) { Some(e) => e, None => { err(tf, SysError::InvalidCap); return; } };

    match ep.send(msg.clone()) {
        Ok(SendResult::Delivered { receiver_tid }) => {
            let recv_id = fjell_abi::task::TaskId::new(receiver_tid, 0);
            if let Some(recv_task) = tasks.get_mut(recv_id) {
                deliver(&mut recv_task.trap_frame, &msg);
            }
            wake(tasks, sched, receiver_tid);
            ok(tf);
            AUDIT.lock_free_append(AuditKindInternal::IpcSend, tidx, receiver_tid as usize, 0);
        }
        Ok(SendResult::Queued) => {
            block(tasks, sched, cur_id);
            ok(tf);
            AUDIT.lock_free_append(AuditKindInternal::IpcSend, tidx, 0, 0);
        }
        Err(e) => { err(tf, e.into()); AUDIT.lock_free_append(AuditKindInternal::IpcDenied, tidx, 0, 0); }
    }
}

pub fn sys_ipc_recv(
    tf: &mut TrapFrame, tidx: usize,
    ct: &mut CapTable, et: &mut EndpointTable,
    tasks: &mut TaskTable, sched: &mut Scheduler,
    cur_id: TaskId,
) {
    if let Err(e) = check_right(tf, tidx, ct, CapRights::RECV) { err(tf, e); return; }
    let ep_h = CapHandle(tf.gpr[10] as u32);
    let (ep_id, recv_lease) = {
        let cs = match ct.cspace(tidx) { Some(c) => c, None => { err(tf, SysError::InternalError); return; } };
        match cs.get(ep_h) { Ok(c) => (c.object_id, c.lease), Err(e) => { err(tf, e); return; } }
    };
    let ep = match et.get_mut(ep_id) { Some(e) => e, None => { err(tf, SysError::InvalidCap); return; } };
    let waiter = match recv_lease {
        Some(lb) => fjell_ipc::endpoint::RecvWaiter::with_lease(tidx as u16, lb),
        None     => fjell_ipc::endpoint::RecvWaiter::no_lease(tidx as u16),
    };
    match ep.recv(waiter) {
        Ok(RecvResult::Delivered(msg)) => {
            if msg.is_call {
                // RFC 034: store the call's lease binding in the reply edge.
                if let Some(lb) = msg.lease {
                    ct.set_reply_with_lease(tidx, msg.sender_tid, lb);
                } else {
                    ct.set_reply(tidx, msg.sender_tid);
                }
                // For ipc_call, the sender waits for an explicit ipc_reply — do NOT
                // wake it here.  Waking prematurely would give the caller stale data
                // and allow it to continue before the server has replied.
            } else {
                // One-way send: sender can proceed immediately.
                wake(tasks, sched, msg.sender_tid);
            }
            deliver(tf, &msg);
            AUDIT.lock_free_append(AuditKindInternal::IpcRecv, tidx, msg.sender_tid as usize, 0);
        }
        Ok(RecvResult::Queued) => {
            block(tasks, sched, cur_id);
            AUDIT.lock_free_append(AuditKindInternal::IpcRecv, tidx, 0, 0);
        }
        Err(e) => err(tf, e.into()),
    }
}

pub fn sys_ipc_call(
    tf: &mut TrapFrame, tidx: usize,
    ct: &mut CapTable, et: &mut EndpointTable,
    tasks: &mut TaskTable, sched: &mut Scheduler,
    cur_id: TaskId,
) {
    if let Err(e) = check_right(tf, tidx, ct, CapRights::CALL) {
        err(tf, e);
        AUDIT.lock_free_append(AuditKindInternal::IpcDenied, tidx, 0, 0);
        return;
    }
    let (ep_id, msg) = match build_msg(tf, tidx, ct, true) { Ok(x) => x, Err(e) => { err(tf, e); return; } };
    let ep = match et.get_mut(ep_id) { Some(e) => e, None => { err(tf, SysError::InvalidCap); return; } };

    match ep.send(msg.clone()) {
        Ok(SendResult::Delivered { receiver_tid }) => {
            let recv_id = fjell_abi::task::TaskId::new(receiver_tid, 0);
            if let Some(recv_task) = tasks.get_mut(recv_id) {
                deliver(&mut recv_task.trap_frame, &msg);
                } else {
                }
            // RFC 034: pass the call's lease binding into the reply edge.
            if let Some(lb) = msg.lease {
                ct.set_reply_with_lease(receiver_tid as usize, tidx as u16, lb);
            } else {
                ct.set_reply(receiver_tid as usize, tidx as u16);
            }
            wake(tasks, sched, receiver_tid);
            block(tasks, sched, cur_id);
            ok(tf);
            AUDIT.lock_free_append(AuditKindInternal::IpcCall, tidx, receiver_tid as usize, 0);
        }
        Ok(SendResult::Queued) => {
            // No receiver yet — message queued; block caller until server recvs
            // and then replies.
            block(tasks, sched, cur_id);
            ok(tf);
            AUDIT.lock_free_append(AuditKindInternal::IpcCall, tidx, 0, 0);
        }
        Err(e) => {
            err(tf, e.into());
            AUDIT.lock_free_append(AuditKindInternal::IpcDenied, tidx, 0, 0);
        }
    }
}

pub fn sys_ipc_reply(
    tf: &mut TrapFrame, tidx: usize,
    ct: &mut CapTable,
    tasks: &mut TaskTable, sched: &mut Scheduler,
) {
    let edge = match ct.take_reply(tidx) { Ok(e) => e, Err(e) => { err(tf, e); return; } };

    // RFC 034: if the call's lease was revoked while the caller was blocked,
    // the reply edge was already cancelled by wake_or_cancel_blocked_ipc_for_lease
    // and the caller was woken with LeaseRevoked.  In that case ct.take_reply()
    // would have returned None (BadState).  This check is defense-in-depth for
    // the case where revoke arrived after take_reply but before delivery.
    if let Some(lb) = edge.lease {
        // SAFETY: capability handle is validated by require_cap before this call.
        let lt = unsafe { crate::get_lease_table() };
        if lt.check_active(lb.lease_id, lb.epoch_at_issue).is_err() {
            // Lease revoked: drop the reply silently.  The caller is already
            // either woken (by wake_or_cancel) or will fail on next use.
            err(tf, SysError::LeaseRevoked);
            return;
        }
    }

    let caller_id = TaskId::new(edge.caller_tid, 0);
    if let Some(caller) = tasks.get_mut(caller_id) {
        // Guard: only deliver reply to a task that is still blocked waiting for it.
        // If the caller already exited or faulted, silently drop the reply.
        if matches!(caller.state, crate::task::tcb::TaskState::Blocked(_)) {
            // Reply label is in a1 (gpr[11]); a0 is the ep handle arg (ignored).
            let reply_label = tf.gpr[REG_A1] as usize;
            caller.trap_frame.gpr[REG_A0] = SysError::Ok as isize as usize;
            caller.trap_frame.gpr[REG_A1] = reply_label;
            for i in 0..4usize { caller.trap_frame.gpr[12 + i] = tf.gpr[12 + i]; }
            caller.state = crate::task::tcb::TaskState::Runnable;
            sched.enqueue_runnable(caller_id, PRIORITY_USER);
        }
    }
    ok(tf);
    AUDIT.lock_free_append(AuditKindInternal::IpcReply, tidx, edge.caller_tid as usize, 0);
}

/// RFC 034: Wake all tasks blocked in IPC whose lease binding matches
/// `(lease_id, old_epoch)`.
///
/// Called by `dispatch_lease_revoke` after `lt.revoke()` returns the new
/// epoch.  `old_epoch = new_epoch - 1` (or the last epoch before revocation).
///
/// Walks all endpoints once (O(MAX_ENDPOINTS × QUEUE_DEPTH)) and the reply
/// table once (O(MAX_TASKS)).  Wakes each cancelled task with `LeaseRevoked`.
pub fn cancel_blocked_ipc_for_lease(
    lease_id:  fjell_abi::lease::LeaseId,
    old_epoch: u32,
    ct:        &mut CapTable,
    et:        &mut EndpointTable,
    tasks:     &mut TaskTable,
    sched:     &mut Scheduler,
) {
    // 1. Cancel blocked senders/receivers across all endpoints.
    for ep_id in 0..super::table::MAX_ENDPOINTS as u32 {
        if let Some(ep) = et.get_mut(ep_id) {
            let cancelled = ep.cancel_by_lease(lease_id, old_epoch);
            // Wake each cancelled sender.
            for &tid in cancelled.senders() {
                wake_with_error(tasks, sched, tid, SysError::LeaseRevoked);
            }
            // Wake each cancelled receiver.
            for &tid in cancelled.receivers() {
                wake_with_error(tasks, sched, tid, SysError::LeaseRevoked);
            }
        }
    }
    // 2. Cancel reply edges (blocked callers waiting for a reply).
    let (caller_tids, n) = ct.cancel_replies_for_lease(lease_id, old_epoch);
    for tid in &caller_tids[..n] {
        wake_with_error(tasks, sched, *tid, SysError::LeaseRevoked);
    }
    AUDIT.lock_free_append(
        AuditKindInternal::IpcDenied,   // closest existing kind for now
        lease_id.0 as usize, old_epoch as usize, 0,
    );
}

/// `sys_ipc_try_recv(a0=ep_handle) -> a0=status [, a1..=message]`
///
/// RFC 019: Non-blocking receive.  Returns `WouldBlock` immediately if no
/// message is pending on the endpoint, without sleeping the calling task.
/// Used by cooperative service loops to avoid deadlock without preemption.
pub fn sys_ipc_try_recv(
    tf:    &mut TrapFrame,
    tidx:  usize,
    ct:    &CapTable,
    et:    &mut EndpointTable,
    table: &crate::task::tcb::TaskTable,
) {
    // Validate endpoint cap (including lease).
    if let Err(e) = check_right(tf, tidx, ct, CapRights::RECV) {
        err(tf, e);
        return;
    }
    let ep_h    = CapHandle(tf.gpr[10] as u32);
    let cs      = match ct.cspace(tidx) { Some(c) => c, None => { err(tf, SysError::InternalError); return; }};
    let cap     = match cs.get(ep_h) { Ok(c) => c, Err(e) => { err(tf, e); return; }};
    let ep      = match et.get_mut(cap.object_id) { Some(e) => e, None => { err(tf, SysError::InvalidCap); return; }};

    use fjell_ipc::endpoint::EndpointError;
    match ep.try_recv() {
        Ok(msg) => deliver(tf, &msg),
        Err(EndpointError::WouldBlock) => err(tf, SysError::WouldBlock),
        Err(_)                         => err(tf, SysError::InternalError),
    }
    let _ = table; // future: per-task stats
}


// ── Helpers ───────────────────────────────────────────────────────────────────

fn wake(tasks: &mut TaskTable, sched: &mut Scheduler, tid: u16) {
    let id = TaskId::new(tid, 0);
    if let Some(t) = tasks.get_mut(id) {
        if matches!(t.state, TaskState::Blocked(_)) {
            t.state = TaskState::Runnable;
            sched.enqueue_runnable(id, PRIORITY_USER);
        }
    }
}

/// Wake a task with a specific error code in `a0` (RFC 034: LeaseRevoked wake).
fn wake_with_error(tasks: &mut TaskTable, sched: &mut Scheduler, tid: u16, e: SysError) {
    let id = TaskId::new(tid, 0);
    if let Some(t) = tasks.get_mut(id) {
        if matches!(t.state, TaskState::Blocked(_)) {
            t.trap_frame.gpr[crate::task::tcb::REG_A0] = e as isize as usize;
            t.state = TaskState::Runnable;
            sched.enqueue_runnable(id, PRIORITY_USER);
        }
    }
}

fn block(tasks: &mut TaskTable, sched: &mut Scheduler, id: TaskId) {
    if let Some(t) = tasks.get_mut(id) {
        t.state = TaskState::Blocked(BlockReason::ReservedForIpc);
    }
    // Suspend the current slot without dequeuing the next task.
    // schedule_next in the trap dispatcher calls choose_next() after this
    // returns.  Calling on_exit() (which internally calls choose_next()) here
    // would pop a task from the ready queue and discard the result, causing
    // that task to be silently skipped.
    sched.suspend_current();
}
