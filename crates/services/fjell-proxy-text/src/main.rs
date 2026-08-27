#![allow(unused_assignments)] // IPC polling idiom: t/w* are overwritten by sys_ipc_recv
#![no_std]
#![no_main]
mod rt;
use fjell_cap::CapHandle;
use fjell_proxy_text::{render_event, render_intent, render_state};
use fjell_semantic_format::{
    ActionId, CorrelationId, IntentNode, SemanticEnvelope, SemanticPayload,
};
use fjell_service_api::proxy_text as proto;
use fjell_service_api::semantic_stream as sem_proto;
use fjell_syscall::{sys_cap_inspect, sys_debug_writeln};

const EP_SLOT: u32 = 0;
// semantic-stream endpoint cap (object 7), pre-installed by spawn.rs
// (RFC-v0.23-001) — used for the DISPATCH_ACTION return leg.
const SEM_STREAM_EP: u32 = 1;
// A deliberately narrow-rights capability (SEND | REPLY only), pre-installed
// by spawn.rs at slot 2. Inspected via sys_cap_inspect (kernel-verified, not
// self-asserted) to obtain the rights presented for each action's check.
const DEMO_CAP_SLOT: u32 = 2;

/// Byte size of one `SemanticEnvelope`, rounded up to a whole number of
/// 32-byte chunks — see fjell_service_api::chunked's doc comment.
const ENV_SIZE: usize = core::mem::size_of::<SemanticEnvelope>();
const ENV_BUF_SIZE: usize = ENV_SIZE.div_ceil(32) * 32;

fn recv_call() -> (usize, usize, usize, usize, usize) {
    let (mut t, mut w0, mut w1, mut w2, mut w3) = (0usize, 0usize, 0usize, 0usize, 0usize);
    // SAFETY: category=raw-pointer-deref IPC call slot is valid; response buffer length is bounded by MAX_IPC_MSG.
    unsafe {
        core::arch::asm!("li a7, 21","ecall", in("a0") EP_SLOT as usize, lateout("a1") t, lateout("a2") w0, lateout("a3") w1, lateout("a4") w2, lateout("a5") w3, lateout("a7") _, options(nostack));
    }
    (t, w0, w1, w2, w3)
}

fn reply(tag: usize, w0: usize, w1: usize, w2: usize) {
    // SAFETY: category=raw-pointer-deref IPC call slot is valid; response buffer length is bounded by MAX_IPC_MSG.
    unsafe {
        core::arch::asm!("li a7, 23","ecall", in("a0") 0usize, in("a1") tag, in("a2") w0, in("a3") w1, in("a4") w2, lateout("a7") _, options(nostack));
    }
}

/// Issue a capability-checked `ActionRequest` for one action back to
/// semantic-stream (RFC-v0.23-001 Slice 3). `granted_rights` comes from
/// `sys_cap_inspect` on `DEMO_CAP_SLOT` — a real, kernel-verified rights
/// bitmask, not a self-asserted claim — so the accept/refuse outcome
/// reflects an actual capability, not a fabricated one.
fn dispatch_action(correlation_id: u64, action_id: ActionId) {
    let granted_rights: u32 = match sys_cap_inspect(CapHandle::new(DEMO_CAP_SLOT as u16, 0)) {
        Ok((_kind, rights, _badge)) => rights as u32,
        Err(_) => 0,
    };
    // DISPATCH_ACTION is a single 4-word call (correlation_id, action_id,
    // granted_rights), not a chunked transfer — sent directly rather than
    // through fjell_service_api::chunked (which always does BEGIN/CHUNK/COMMIT).
    let result = ipc_call_action(
        SEM_STREAM_EP,
        sem_proto::DISPATCH_ACTION,
        correlation_id as usize,
        action_id.0 as usize,
        granted_rights as usize,
    );
    if result == fjell_semantic_format::EventResult::Ok as usize {
        sys_debug_writeln("proxy-text: action accepted");
    } else if result == fjell_semantic_format::EventResult::Denied as usize {
        sys_debug_writeln("proxy-text: action DENIED (capability not held)");
    } else {
        sys_debug_writeln("proxy-text: action result: not applicable");
    }
}

/// Single (non-chunked) 4-word blocking IPC call — used for DISPATCH_ACTION,
/// which fits entirely in one message (correlation_id, action_id,
/// granted_rights).
fn ipc_call_action(ep_slot: u32, tag: usize, w0: usize, w1: usize, w2: usize) -> usize {
    let reply_tag: usize;
    let result_word: usize;
    // SAFETY: category=raw-pointer-deref IPC call slot is valid; register constraints match the Fjell syscall ABI (4-word ipc_call, RFC-v0.23-001).
    //
    // `sys_ipc_reply` (crates/fjell-kernel/src/cap/syscall.rs) copies all
    // four reply words (a2..a5) into the caller's trap frame unconditionally
    // — the reply tag carries no word-count packing, unlike a call/send tag.
    // This must capture a2 as a lateout to read the reply's data word (the
    // `EventResult`); an earlier version only captured a1 (the reply tag)
    // and silently discarded the result, which is why every action came
    // back "not applicable" despite the lookup succeeding on the server side.
    unsafe {
        core::arch::asm!(
            "li a7, 22", "ecall",
            inlateout("a0") ep_slot as usize => _,
            inlateout("a1") tag | (4usize << 16) => reply_tag,
            inlateout("a2") w0 => result_word,
            in("a3") w1, in("a4") w2, in("a5") 0usize,
            lateout("a7") _,
            options(nostack),
        );
    }
    let _ = reply_tag;
    result_word
}

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    // RFC-0.26-004 (closes E-020/E-021): this used to announce readiness by
    // sending `proto::READY` into this task's own endpoint (object 8) — a
    // message nothing has consumed since `init`'s `wait_ready_exact` was
    // removed (see `fjell-init/src/main.rs`'s M5 section): under the
    // established invariant, this endpoint's only receiver is proxy-text
    // itself, so a self-addressed announcement has no reader.
    //
    // It is also actively harmful, not merely dead: `sys_ipc_send`'s
    // one-way path blocks the caller when the message queues with no
    // receiver waiting (`crates/fjell-kernel/src/cap/syscall.rs` —
    // `sys_ipc_send`'s `SendResult::Queued` arm calls `block()`, though its
    // own doc comment on `sys_ipc_try_send` describes a non-blocking
    // contract). Calling `send_ready()` here — before this task has reached
    // its own `recv_call()` — queues the message against the very endpoint
    // only this task can ever drain, self-deadlocking permanently. This was
    // previously masked by `init` also holding a receive capability here
    // and reaching `wait_ready_exact` first, giving the send an immediate
    // receiver; removing `init` as a receiver (this RFC) removes that
    // accidental cover. Filed as a new erratum — see review request.
    sys_debug_writeln("M5: proxy-text started");

    let mut buf = [0u8; ENV_BUF_SIZE];
    let mut offset = 0usize;

    loop {
        let (tag_packed, w0, w1, w2, w3) = recv_call();
        let tag = tag_packed & 0xFFFF;
        match tag {
            t if t == proto::RENDER_BEGIN => {
                offset = 0;
                reply(proto::RENDER_OK, 0, 0, 0);
            }
            t if t == proto::RENDER_CHUNK => {
                fjell_service_api::chunked::write_chunk(&mut buf, offset, w0, w1, w2, w3);
                offset += fjell_service_api::chunked::CHUNK_BYTES;
                reply(proto::RENDER_OK, 0, 0, 0);
            }
            t if t == proto::RENDER_COMMIT => {
                // `buf` holds ENV_BUF_SIZE >= ENV_SIZE bytes, every byte
                // either real transferred data or the sender's
                // zero-padding — see fjell_service_api::chunked::reassemble
                // for the safety reasoning (shared with the send side in
                // fjell-semantic-stream/src/main.rs).
                let envelope: SemanticEnvelope = fjell_service_api::chunked::reassemble(&buf);
                // Render (and, for an Intent, copy out what the return leg
                // needs) BEFORE replying. `dispatch_action` below calls back
                // into semantic-stream — which is, right now, still blocked
                // waiting for *this* RENDER_COMMIT reply. Issuing that call
                // before replying would deadlock (semantic-stream can't
                // service a new incoming call while blocked on its own
                // pending one); confirmed live as a silent hang, not a
                // fault, before this fix. So: reply first, unblocking
                // semantic-stream, then dispatch actions as an independent
                // follow-up round trip.
                let pending_actions: Option<(u64, IntentNode)> = match &envelope.payload {
                    SemanticPayload::State(n) => {
                        render_state(n);
                        None
                    }
                    SemanticPayload::Event(n) => {
                        render_event(n);
                        None
                    }
                    SemanticPayload::Intent(n) => {
                        render_intent(n);
                        let correlation_id: u64 = match envelope.correlation_id {
                            Some(CorrelationId(c)) => c,
                            None => envelope.sequence,
                        };
                        Some((correlation_id, *n))
                    }
                };
                reply(proto::RENDER_OK, 0, 0, 0);
                if let Some((correlation_id, intent)) = pending_actions {
                    for action in intent.actions.iter() {
                        dispatch_action(correlation_id, action.action_id);
                    }
                }
            }
            _ => reply(proto::ERR, 0, 0, 0),
        }
    }
}
