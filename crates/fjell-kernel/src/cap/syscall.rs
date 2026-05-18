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
    let rights = CapRights(tf.gpr[12] as u32);
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
    let h = CapHandle(tf.gpr[10] as u32);
    let cs = match ct.cspace(tidx) { Some(c) => c, None => { err(tf, SysError::InternalError); return; } };
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
    }))
}

fn check_right(tf: &TrapFrame, tidx: usize, ct: &CapTable, right: CapRights) -> Result<(), SysError> {
    let ep_h = CapHandle(tf.gpr[10] as u32);
    let cs   = ct.cspace(tidx).ok_or(SysError::InternalError)?;
    let cap  = cs.get(ep_h)?;
    if !cap.rights.contains(right) { Err(SysError::PermissionDenied) } else { Ok(()) }
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
    if let Err(e) = check_right(tf, tidx, ct, CapRights::SEND) { err(tf, e); return; }
    let (ep_id, msg) = match build_msg(tf, tidx, ct, false) { Ok(x) => x, Err(e) => { err(tf, e); return; } };
    let ep = match et.get_mut(ep_id) { Some(e) => e, None => { err(tf, SysError::InvalidCap); return; } };

    match ep.send(msg) {
        Ok(SendResult::Delivered { receiver_tid }) => {
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
    let ep_id = {
        let ep_h = CapHandle(tf.gpr[10] as u32);
        let cs   = match ct.cspace(tidx) { Some(c) => c, None => { err(tf, SysError::InternalError); return; } };
        match cs.get(ep_h) { Ok(c) => c.object_id, Err(e) => { err(tf, e); return; } }
    };
    let ep = match et.get_mut(ep_id) { Some(e) => e, None => { err(tf, SysError::InvalidCap); return; } };

    match ep.recv(tidx as u16) {
        Ok(RecvResult::Delivered(msg)) => {
            if msg.is_call {
                ct.set_reply(tidx, msg.sender_tid);
                // For ipc_call, the sender waits for an explicit ipc_reply — do NOT
                // wake it here.  Waking prematurely would give the caller stale data
                // and allow it to continue before the server has replied.
            } else {
                // One-way send: sender can proceed immediately.
                wake(tasks, sched, msg.sender_tid);
            }
            deliver(tf, &msg);
            crate::kprintln!("server: request received");
            AUDIT.lock_free_append(AuditKindInternal::IpcRecv, tidx, msg.sender_tid as usize, 0);
        }
        Ok(RecvResult::Queued) => {
            block(tasks, sched, cur_id);
            crate::kprintln!("server: recv waiting");
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
        crate::kprintln!("denied: ipc denied as expected");
        crate::kprintln!("audit: capability.denied");
        AUDIT.lock_free_append(AuditKindInternal::IpcDenied, tidx, 0, 0);
        return;
    }
    let (ep_id, msg) = match build_msg(tf, tidx, ct, true) { Ok(x) => x, Err(e) => { err(tf, e); return; } };
    let ep = match et.get_mut(ep_id) { Some(e) => e, None => { err(tf, SysError::InvalidCap); return; } };

    match ep.send(msg) {
        Ok(SendResult::Delivered { receiver_tid }) => {
            // A receiver was already waiting — deliver directly and block caller
            // until the server calls ipc_reply.
            ct.set_reply(receiver_tid as usize, tidx as u16);
            wake(tasks, sched, receiver_tid);
            block(tasks, sched, cur_id);
            ok(tf);
            crate::kprintln!("client: call sent");
            crate::kprintln!("server: request received");
            crate::kprintln!("audit: ipc.call");
            AUDIT.lock_free_append(AuditKindInternal::IpcCall, tidx, receiver_tid as usize, 0);
        }
        Ok(SendResult::Queued) => {
            // No receiver yet — message queued; block caller until server recvs
            // and then replies.
            block(tasks, sched, cur_id);
            ok(tf);
            crate::kprintln!("client: call sent");
            crate::kprintln!("audit: ipc.call");
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
    let caller_id = TaskId::new(edge.caller_tid, 0);
    if let Some(caller) = tasks.get_mut(caller_id) {
        // Guard: only deliver reply to a task that is still blocked waiting for it.
        // If the caller already exited or faulted, silently drop the reply.
        if matches!(caller.state, crate::task::tcb::TaskState::Blocked(_)) {
            let tag_raw = tf.gpr[10] as usize;
            caller.trap_frame.gpr[REG_A0] = SysError::Ok as isize as usize;
            caller.trap_frame.gpr[REG_A1] = tag_raw;
            for i in 0..4usize { caller.trap_frame.gpr[12 + i] = tf.gpr[11 + i]; }
            caller.state = crate::task::tcb::TaskState::Runnable;
            sched.enqueue_runnable(caller_id, PRIORITY_USER);
            crate::kprintln!("server: reply sent");
            crate::kprintln!("client: reply received");
            crate::kprintln!("audit: ipc.reply");
        }
    }
    ok(tf);
    AUDIT.lock_free_append(AuditKindInternal::IpcReply, tidx, edge.caller_tid as usize, 0);
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
