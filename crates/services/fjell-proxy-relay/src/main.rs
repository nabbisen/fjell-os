//! `proxy-relay` — asks `semantic-stream` for messages on `proxy-text`'s behalf
//! (RFC-0.34-001 D5/D8).
//!
//! `proxy-text` speaks a blocking protocol: the sender calls, it renders, it
//! replies. That is exactly what a stream must not do to a presentation — a
//! presentation that is absent, slow or dead would hold the stream, and every
//! publisher behind it (E-058). So the blocking is moved here. This task asks
//! the stream for its next message (the stream answers at once; see
//! `fjell_service_api::presentation`), forwards it to `proxy-text` with the
//! same blocking call `proxy-text` has always answered, and asks again.
//!
//! If `proxy-text` never started, or faults, this task blocks in the forward
//! and never asks again. Nothing else waits on this task: the stream sees "this
//! presentation stopped asking", queues up to its bound, drops after it, and
//! says so. `proxy-text`'s source is untouched.
//!
//! There is nothing in this file that can fault between being told "parked" and
//! reaching `recv`, on purpose: that window is the one place the stream can
//! block on a presentation (its wake), so the code in it is one call.

#![no_std]
#![no_main]
mod rt;

use fjell_service_api::{chunked, presentation};
use fjell_syscall::sys_ipc_recv_msg;

/// This task's own endpoint (object 13): where the stream's wake arrives.
const EP_SLOT: u32 = 0;
/// A CALL capability to `semantic-stream` (object 7).
const STREAM_EP: u32 = 1;
/// A CALL capability to `proxy-text` (object 8).
const PROXY_TEXT_EP: u32 = 2;

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    loop {
        match presentation::ask(STREAM_EP) {
            presentation::Ask::Message { tag, words } => {
                // Same tag and words, so `proxy-text` receives exactly what
                // `chunked::send` used to hand it. Its reply is not needed:
                // there is nothing to do with `RENDER_OK` here, and a refusal
                // (`ERR`) is `proxy-text`'s own framing check speaking.
                let _ =
                    chunked::ipc_call4(PROXY_TEXT_EP, tag, words[0], words[1], words[2], words[3]);
            }
            presentation::Ask::Empty => {
                // Parked. The stream sends one wake when there is something;
                // whatever arrives, ask again.
                let _ = sys_ipc_recv_msg(EP_SLOT);
            }
        }
    }
}
