//! Semantic Stream Service — M5.
//!
//! publish / subscribe / validate / action dispatch for semantic nodes.

#![allow(unused_assignments)] // IPC polling idiom: t/w* are overwritten by sys_ipc_recv
#![no_std]
#![no_main]
mod rt;

use fjell_semantic_format::*;
use fjell_syscall::sys_debug_writeln;

// ── Validation ────────────────────────────────────────────────────────────────
//
// RFC-v0.23-001 finding: this file previously kept three 32-slot rings
// (`INTENT_RING`/`STATE_RING`/`EVENT_RING`) as `static` `UnsafeCell`s,
// written via `publish()`. Two independent defects, both now removed:
//
// 1. Sizing: `SemanticEnvelope` is kilobyte-scale (dominated by
//    `IntentNode`/`StateNode`'s `FixedVec` fields). At 32 slots, the three
//    rings reserved roughly 470 KiB of BSS. This was invisible before this
//    RFC because nothing ever called `publish()` over IPC, so the linker
//    dead-code-eliminated the unreachable statics entirely — wiring the
//    service loop up for real (this slice) made them reachable, and the
//    true size failed `sys_task_spawn` with `NoMemory` at boot.
// 2. Writability: even at a trivially small size, writing to *any* static
//    (via `UnsafeCell` or otherwise) is impossible here. `spawn.rs` maps a
//    service's entire image (text, data, bss alike) `R | X | U` —
//    deliberately no `W`, since a page cannot be both writable and
//    executable — so any write to `INTENT_RING` et al. faults
//    (`StorePageFault`, confirmed live). This is the exact constraint the
//    handoff's design decision #4 already documents for `proxy-text`'s
//    `ProxyState` ("`static mut` is forbidden in services — BSS-write page
//    faults in `no_std` RISC-V"); it applies equally to a `static
//    UnsafeCell`, which was never exercised enough to be caught before.
//
// Neither is a boundary/protocol decision — both are pre-existing,
// previously-latent defects in an already-in-scope file, fixed here so the
// service can run at all. Nothing else in this RFC reads from these rings
// (there is no subscribe/query IPC handler), so the fix is to stop writing
// to them rather than to relocate them: validation is kept, storage is
// not. The one thing Slice 3 needs — the last intent's `ActionSpec`s, to
// look up `required_capability` when proxy-text calls back — is instead
// kept loop-local in `service_main` (a plain stack variable, matching the
// same loop-local pattern design decision #4 already uses).

pub fn validate_envelope(env: &SemanticEnvelope) -> bool {
    match &env.payload {
        SemanticPayload::Intent(n) => validate_intent(n).is_ok(),
        SemanticPayload::State(n) => validate_state(n).is_ok(),
        SemanticPayload::Event(_) => true,
    }
}

/// RFC-v0.23-001 Slice 3: real capability check, using the intent's own
/// declared `required_capability` as the source of truth rather than a
/// hardcoded rule (architect ruling, review-record-design-conflict.md §1).
/// `granted_rights` is the caller's kernel-verified rights bitmask (read via
/// `sys_cap_inspect` on the caller's side, not self-asserted).
fn dispatch_action_checked(
    intent: &IntentNode,
    action_id: ActionId,
    correlation_id: CorrelationId,
    granted_rights: u32,
) -> ActionResult {
    let spec = intent.actions.iter().find(|a| a.action_id == action_id);
    let Some(spec) = spec else {
        return ActionResult {
            correlation_id,
            result: EventResult::NotApplicable,
            message: TextToken::new("unknown action_id"),
        };
    };
    let required = spec.required_capability.map(|c| c.rights).unwrap_or(0);
    // required ⊆ granted, i.e. no required bit is missing from granted.
    let allowed = required & !granted_rights == 0;
    if allowed {
        ActionResult {
            correlation_id,
            result: EventResult::Ok,
            message: TextToken::new("action accepted"),
        }
    } else {
        ActionResult {
            correlation_id,
            result: EventResult::Denied,
            message: TextToken::new("required capability not held"),
        }
    }
}

// ── IPC (recoveryd pattern, RFC-v0.23-001) ─────────────────────────────────────

use fjell_service_api::semantic_stream as proto;

const EP_SLOT: u32 = 0;
// proxy-text endpoint cap (object 8), pre-installed by spawn.rs.
const PROXY_TEXT_EP: u32 = 1;

/// Byte size of one `SemanticEnvelope`, rounded up to a whole number of
/// 32-byte chunks so every chunked write lands fully in bounds (the tail
/// past the real struct size is zero-padding, matching what the sender
/// already transmits — see `fjell_service_api::chunked`).
const ENV_SIZE: usize = core::mem::size_of::<SemanticEnvelope>();
const ENV_BUF_SIZE: usize = ENV_SIZE.div_ceil(32) * 32;

fn send_ready() {
    // SAFETY: category=raw-pointer-deref IPC call slot is valid; response buffer length is bounded by MAX_IPC_MSG.
    unsafe {
        core::arch::asm!("li a7, 20","ecall", in("a0") EP_SLOT as usize, in("a1") proto::READY, lateout("a0") _, lateout("a7") _, options(nostack));
    }
}

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

/// Forward `bytes` (a full `SemanticEnvelope`'s raw bytes) on to proxy-text.
fn forward_to_proxy_text(bytes: &[u8]) -> usize {
    use fjell_service_api::proxy_text as pt_proto;
    fjell_service_api::chunked::send(
        PROXY_TEXT_EP,
        pt_proto::RENDER_BEGIN,
        pt_proto::RENDER_CHUNK,
        pt_proto::RENDER_COMMIT,
        bytes,
    )
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    send_ready();
    sys_debug_writeln("M5: semantic-stream started");
    sys_debug_writeln("M5: semantic policy loaded");

    // RFC-v0.23-001: chunked-receive state and the last published intent
    // (needed to look up an action's required_capability when proxy-text
    // later calls back with DISPATCH_ACTION). Plain locals — this loop is
    // the only thing that ever touches them, so no static/UnsafeCell needed.
    let mut buf = [0u8; ENV_BUF_SIZE];
    let mut offset = 0usize;
    let mut last_intent: Option<IntentNode> = None;

    loop {
        let (tag_packed, w0, w1, w2, w3) = recv_call();
        let tag = tag_packed & 0xFFFF;
        match tag {
            t if t == proto::PUBLISH_BEGIN => {
                offset = 0;
                reply(proto::PUBLISH_OK, 0, 0, 0);
            }
            t if t == proto::PUBLISH_CHUNK => {
                fjell_service_api::chunked::write_chunk(&mut buf, offset, w0, w1, w2, w3);
                offset += fjell_service_api::chunked::CHUNK_BYTES;
                reply(proto::PUBLISH_OK, 0, 0, 0);
            }
            t if t == proto::PUBLISH_COMMIT => {
                // `buf` holds ENV_BUF_SIZE >= ENV_SIZE bytes, every byte
                // either real transferred data or the sender's
                // zero-padding — see fjell_service_api::chunked::reassemble
                // for the safety reasoning.
                let envelope: SemanticEnvelope = fjell_service_api::chunked::reassemble(&buf);
                if let SemanticPayload::Intent(n) = &envelope.payload {
                    last_intent = Some(*n);
                }
                let ok = validate_envelope(&envelope);
                if ok {
                    let _ = forward_to_proxy_text(&buf[..]);
                    reply(proto::PUBLISH_OK, 0, 0, 0);
                } else {
                    reply(proto::PUBLISH_ERR, 0, 0, 0);
                }
            }
            t if t == proto::DISPATCH_ACTION => {
                let correlation_id = CorrelationId(w0 as u64);
                let action_id = ActionId(w1 as u16);
                let granted_rights = w2 as u32;
                let result = match &last_intent {
                    Some(intent) => {
                        dispatch_action_checked(intent, action_id, correlation_id, granted_rights)
                    }
                    None => ActionResult {
                        correlation_id,
                        result: EventResult::NotApplicable,
                        message: TextToken::new("no intent on record"),
                    },
                };
                reply(proto::ACTION_RESULT, result.result as usize, 0, 0);
            }
            _ => reply(proto::ERR, 0, 0, 0),
        }
    }
}
