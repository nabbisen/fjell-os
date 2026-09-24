#![allow(unused_assignments)] // IPC polling idiom: t/w* are overwritten by sys_ipc_recv
#![no_std]
#![no_main]
mod rt;
use fjell_cap::CapHandle;
use fjell_proxy_text::{render_event, render_intent, render_state};
use fjell_semantic_format::{ActionId, CorrelationId, SemanticPayload, wire};
use fjell_service_api::presentation;
use fjell_service_api::proxy_text as proto;
use fjell_service_api::semantic_stream as sem_proto;
use fjell_syscall::{sys_cap_inspect, sys_debug_writeln, sys_ipc_recv_msg};

/// This task's own endpoint (object 8): where the stream's wake arrives.
const EP_SLOT: u32 = 0;
// semantic-stream endpoint cap (object 7), pre-installed by spawn.rs
// (RFC-v0.23-001) — used to ASK for the next message (RFC-0.34-001 D8/D9) and
// for the DISPATCH_ACTION return leg.
const SEM_STREAM_EP: u32 = 1;
// A deliberately narrow-rights capability (SEND | REPLY only), pre-installed
// by spawn.rs at slot 2. Inspected via sys_cap_inspect (kernel-verified, not
// self-asserted) to obtain the rights presented for each action's check.
const DEMO_CAP_SLOT: u32 = 2;

/// Receive-buffer size: the widest envelope the wire format can carry
/// (RFC-0.32-002 D1), rounded up to a whole number of 32-byte chunks — not
/// `size_of::<SemanticEnvelope>()`, which is the struct's in-memory size and
/// has nothing to do with its encoding.
const ENV_BUF_SIZE: usize = wire::MAX_WIRE_BYTES.div_ceil(32) * 32;

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
///
/// RFC-0.28-002 (E-032 audit, kept — escalated, not deleted): 4-word
/// `IpcCall` has no `fjell-syscall` wrapper (`sys_ipc_call_words` covers
/// only 3) — see the governing RFC's answer document. `a3`/`a4`/`a5` are
/// now declared `inlateout` alongside `a2`: this file's own history already
/// records the `a2` half of this exact bug (see the comment below), and the
/// same defect was live, unfixed, in the other three reply-word registers.
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
            inlateout("a3") w1 => _, inlateout("a4") w2 => _, inlateout("a5") 0usize => _,
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
    // sending `proto::READY` into this task's own endpoint — a message nothing
    // consumed, and one that self-deadlocked (a one-way send blocks when no
    // receiver waits; E-022, closed by RFC-0.27-002). Only the init line below
    // announces now.
    sys_debug_writeln("M5: proxy-text started");

    let mut frames = fjell_service_api::chunked::Reassembler::<ENV_BUF_SIZE>::new();

    // RFC-0.34-001 D8/D9: this service ASKS the stream for its messages and is
    // answered by reply (`fjell_service_api::presentation`), instead of being
    // called by it. The stream therefore never initiates a blocking IPC to a
    // presentation: one that never starts, or faults, cannot stall a publisher
    // (E-058). Until D9 a separate relay task asked on this service's behalf,
    // because this loop answered a blocking protocol; that was a permanent shim
    // to avoid roughly ten lines here.
    //
    // Nothing between being told "parked" and reaching `recv` below can fault
    // but this one call: that window is the one place the stream can wait on a
    // presentation (its wake), and it is a named survivor of E-058.
    loop {
        match presentation::ask(SEM_STREAM_EP) {
            presentation::Ask::Message { tag, words } => match tag {
                t if t == proto::RENDER_BEGIN => {
                    // RFC-0.32-002 D3: the declared length is recorded and
                    // checked, not discarded.
                    let _ = frames.begin(words[0]);
                }
                t if t == proto::RENDER_CHUNK => {
                    if frames
                        .chunk(words[0], words[1], words[2], words[3])
                        .is_err()
                    {
                        frames.reset();
                    }
                }
                t if t == proto::RENDER_COMMIT => {
                    let decoded = match frames.commit() {
                        Ok(bytes) => wire::decode_exact(bytes).ok(),
                        Err(_) => None,
                    };
                    frames.reset();
                    let Some(envelope) = decoded else {
                        // Framing or decoding refused it. Say so, as the
                        // braille presentation does, rather than show nothing.
                        sys_debug_writeln("proxy-text: envelope refused");
                        continue;
                    };
                    match &envelope.payload {
                        SemanticPayload::State(n) => render_state(n),
                        SemanticPayload::Event(n) => render_event(n),
                        SemanticPayload::Intent(n) => {
                            render_intent(n);
                            let correlation_id: u64 = match envelope.correlation_id {
                                Some(CorrelationId(c)) => c,
                                None => envelope.sequence,
                            };
                            // The return leg is an independent round trip to the
                            // stream. It used to have to wait until this task had
                            // replied to the stream's blocked forward, or both
                            // deadlocked (confirmed live); the stream no longer
                            // waits on this task at all.
                            for action in n.actions.iter() {
                                dispatch_action(correlation_id, action.action_id);
                            }
                        }
                    }
                }
                _ => {}
            },
            presentation::Ask::Empty => {
                // Parked. The stream sends one wake when there is something;
                // whatever arrives, ask again.
                let _ = sys_ipc_recv_msg(EP_SLOT);
            }
        }
    }
}
