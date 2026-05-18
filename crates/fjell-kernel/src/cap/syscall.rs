//! Capability and IPC syscall handlers for M3.

#![allow(dead_code)]

use fjell_abi::error::SysError;
use fjell_cap::{CapHandle, CapKind, CapRights};
use fjell_ipc::endpoint::{PendingMessage, RecvWaiter, SendResult, RecvResult};
use fjell_ipc::message::{MessageTag, IPC_WORDS};
use crate::{
    audit::ring::{AuditKindInternal, AUDIT},
    task::{
        scheduler::{Scheduler, PRIORITY_USER},
        tcb::{BlockReason, TaskState, TaskTable, TrapFrame, REG_A0, REG_A1},
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
    // RFC 015: validate source cap lease before copying.
    {
        let lt = unsafe { crate::get_lease_table() };
        let cs = match ct.cspace(tidx) { Some(c) => c, None => { err(tf, SysError::InternalError); return; }};
        let cap = match cs.get(src) { Ok(c) => c, Err(e) => { err(tf, e); return; }};
        if let Err(e) = cap.check_lease(lt) { err(tf, e.into()); return; }
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
    // RFC 015: validate source cap lease before minting a derived capability.
    {
        let lt = unsafe { crate::get_lease_table() };
        let cs = match ct.cspace(tidx) { Some(c) => c, None => { err(tf, SysError::InternalError); return; }};
        let cap = match cs.get(src) { Ok(c) => c, Err(e) => { err(tf, e); return; }};
        if let Err(e) = cap.check_lease(lt) { err(tf, e.into()); return; }
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
    let cs = match ct.cspace_mut(tidx) { Some(c) => c, None => { err(tf, SysError::InternalError); return; } };
    match cs.revoke(h) { Ok(()) => ok(tf), Err(e) => err(tf, e) }
    AUDIT.lock_free_append(AuditKindInternal::CapRevoke, h.0 as usize, 0, 0);
}

pub fn sys_cap_inspect(tf: &mut TrapFrame, tidx: usize, ct: &CapTable) {
    let h  = CapHandle(tf.gpr[10] as u32);
    let lt = unsafe { crate::get_lease_table() };
    let cs = match ct.cspace(tidx) { Some(c) => c, None => { err(tf, SysError::InternalError); return; } };
    // RFC 015: validate lease before exposing cap metadata.
    match cs.get(h) {
        Ok(cap) => { if let Err(e) = cap.check_lease(lt) { err(tf, e.into()); return; } }
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

    Ok((cap.object_id, PendingMessage {
        tag,
        sender_tid:   tidx as u16,
        sender_badge: cap.badge,
        words,
        cap_present: false, cap_kind: 0, cap_obj_id: 0, cap_rights: 0,
        is_call,
        lease: cap.lease,   // RFC 034: carry lease binding for revoke wake path
    }))
}

fn check_right(tf: &TrapFrame, tidx: usize, ct: &CapTable, right: CapRights) -> Result<(), SysError> {
    let ep_h = CapHandle(tf.gpr[10] as u32);
    let cs   = ct.cspace(tidx).ok_or(SysError::InternalError)?;
    let cap  = cs.get(ep_h)?;
    if !cap.rights.contains(right) { return Err(SysError::PermissionDenied); }
    // RFC 015: validate lease binding — revoked caps must not be used for IPC.
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

/// Deliver a PendingMessage into the current task's TrapFrame (for recv/call).
fn deliver(tf: &mut TrapFrame, msg: &PendingMessage) {
    ok(tf);
    let packed = (msg.tag.label as usize)
        | ((msg.tag.words as usize) << 16)
        | ((msg.tag.caps  as usize) << 24);
    tf.gpr[REG_A1] = packed;
    tf.gpr[12]     = msg.sender_badge as usize;
    for i in 0..(msg.tag.words as usize).min(4) { tf.gpr[13 + i] = msg.words[i] as usize; }
}

pub fn sys_ipc_send(
    tf: &mut TrapFrame, tidx: usize,
    ct: &mut CapTable, et: &mut EndpointTable,
    tasks: &mut TaskTable, sched: &mut Scheduler,
    cur_id: TaskId,
) {
    // Diagnostic: print "S<tidx>" to UART when IpcSend is entered.
    unsafe {
        let uart = 0x1000_0000usize as *mut u8;
        uart.write_volatile(b'S');
        uart.write_volatile(b'0' + tidx as u8);
        uart.write_volatile(b'a');
        uart.write_volatile(tf.gpr[10] as u8 + b'0'); // cap slot
    }
    if let Err(e) = check_right(tf, tidx, ct, CapRights::SEND) {
        unsafe {
            let uart = 0x1000_0000usize as *mut u8;
            uart.write_volatile(b'F');
        }
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
        let lt = unsafe { crate::get_lease_table() };
        use fjell_cap::slot::LeaseChecker;
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
