//! `svc-presentation-liar` — a test-only presentation that claims rights it
//! does not hold (RFC-0.34-004 D5/R6).
//!
//! E-059's failing case, as a committed tier rather than a scratch edit (see
//! `rfcs/answers/RFC-0.34-004-a-word-that-claims-an-authority-answer.md` for
//! the scratch transcript this tier replaces as the ongoing, gated check).
//! This task is installed with **no** dispatch capability — its row in
//! `fjell-semantic-stream`'s `PRESENTATIONS` grants it `0` — yet it calls
//! `DISPATCH_ACTION` for a dedicated test intent `init` publishes just before
//! spawning it (action 1, requiring `MMIO_MAP`), claiming every right there
//! is. Before RFC-0.34-004, the stream believed the claim; after, the claim
//! is never read — the policy is this task's own compiled-in row, which
//! grants it nothing, so the action is refused regardless of what it claims.
//! `init` starts it only on the console byte `L`, so no other profile has
//! it.

#![no_std]
#![no_main]
mod rt;

use fjell_syscall::{sys_debug_writeln, sys_ipc_recv_msg};

/// This task's own endpoint: unused. This presentation never calls
/// `presentation::ask`, so the stream never wakes it; it parks here forever
/// once its one call is made.
const EP_SLOT: u32 = 0;
/// A CALL capability to `semantic-stream` (object 7).
const STREAM_EP: u32 = 1;
/// A forged claim: every right there is. This task's row in
/// `fjell-semantic-stream::PRESENTATIONS` grants it `0` regardless — nothing
/// it sends can change what the receiver looks up.
const FORGED_RIGHTS: usize = 0xFFFF_FFFF;
/// `init`'s dedicated test intent's action 1, which requires `MMIO_MAP`
/// (`fjell-init/src/main.rs::presentation_liar_test`) — a right this task
/// cannot hold under any honest inspection, since it is installed with no
/// capability beyond the stream endpoint.
const ACTION_REQUIRING_MMIO_MAP: usize = 1;

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("svc-presentation-liar: claiming rights it does not hold");
    let result = ipc_call_action(STREAM_EP, 1, ACTION_REQUIRING_MMIO_MAP, FORGED_RIGHTS);
    if result == fjell_semantic_format::EventResult::Ok as usize {
        sys_debug_writeln("svc-presentation-liar: forged claim ACCEPTED");
    } else {
        sys_debug_writeln("svc-presentation-liar: forged claim refused");
    }
    loop {
        // Nothing more to do, and nobody wakes a presentation that never
        // asked — this blocks forever.
        let _ = sys_ipc_recv_msg(EP_SLOT);
    }
}

/// Single (non-chunked) 4-word blocking IPC call for `DISPATCH_ACTION` —
/// identical in shape to `fjell-proxy-text`'s `ipc_call_action` (RFC-0.28-002,
/// E-032 audit: `sys_ipc_call_words` only covers 3 words, this call needs the
/// tag plus 3 words **and** the reply's data word, which that wrapper
/// discards).
fn ipc_call_action(ep_slot: u32, w0: usize, w1: usize, w2: usize) -> usize {
    let reply_tag: usize;
    let result_word: usize;
    // SAFETY: category=raw-pointer-deref IPC call slot is valid; register constraints match the Fjell syscall ABI (4-word ipc_call, identical to fjell-proxy-text's ipc_call_action).
    unsafe {
        core::arch::asm!(
            "li a7, 22", "ecall",
            inlateout("a0") ep_slot as usize => _,
            inlateout("a1") fjell_service_api::semantic_stream::DISPATCH_ACTION | (4usize << 16) => reply_tag,
            inlateout("a2") w0 => result_word,
            inlateout("a3") w1 => _, inlateout("a4") w2 => _, inlateout("a5") 0usize => _,
            lateout("a7") _,
            options(nostack),
        );
    }
    let _ = reply_tag;
    result_word
}
