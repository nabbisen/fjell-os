//! `proxy-braille` — the second presentation (RFC-0.34-001).
//!
//! It renders each envelope as lines of uncontracted braille patterns on the
//! console: **the stream a braille display driver would consume**, and nothing
//! more. QEMU `virt` has no braille display and none was driven (E-004); this
//! is not "Fjell supports braille" and no output, comment or marker may say so
//! (D7).
//!
//! It is a thin adapter over two things that already exist and are not copied:
//!
//! * the **same decoder** `proxy-text` uses — `wire::decode_exact` behind the
//!   same `chunked::Reassembler` (D1: a second decoder would disprove the
//!   line's own point);
//! * the pure renderer, `fjell_braille::render_lines`, whose vectors run in
//!   Gate 1.
//!
//! It **asks** the stream for its messages and never receives an unsolicited
//! one but a wake (see `fjell_service_api::presentation`): that is what lets
//! the stream stay decoupled from it (D8). It never calls the stream for
//! anything else — there is **no input path** (D6, ADR-v0.5-005), so it
//! dispatches no action and holds no capability that could.
//!
//! Between being told "parked" and reaching its `recv`, the only code that runs
//! is one call. That window is the one place the stream can wait on a
//! presentation; nothing here that can fault is in it.

#![no_std]
#![no_main]
mod rt;

use fjell_semantic_format::wire;
use fjell_service_api::{presentation, proxy_text as msg};
use fjell_syscall::{sys_debug_write, sys_debug_writeln, sys_ipc_recv_msg};

/// This task's own endpoint (object 14): where the stream's wake arrives.
const EP_SLOT: u32 = 0;
/// A CALL capability to `semantic-stream` (object 7).
const STREAM_EP: u32 = 1;

/// The widest envelope the wire format can carry, rounded up to whole chunks —
/// the same sizing `proxy-text` and the stream use.
const ENV_BUF_SIZE: usize = wire::MAX_WIRE_BYTES.div_ceil(32) * 32;

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    let mut frames = fjell_service_api::chunked::Reassembler::<ENV_BUF_SIZE>::new();
    loop {
        match presentation::ask(STREAM_EP) {
            presentation::Ask::Message { tag, words } => match tag {
                t if t == msg::RENDER_BEGIN => {
                    let _ = frames.begin(words[0]);
                }
                t if t == msg::RENDER_CHUNK => {
                    if frames
                        .chunk(words[0], words[1], words[2], words[3])
                        .is_err()
                    {
                        frames.reset();
                    }
                }
                t if t == msg::RENDER_COMMIT => {
                    let decoded = match frames.commit() {
                        Ok(bytes) => wire::decode_exact(bytes).ok(),
                        Err(_) => None,
                    };
                    frames.reset();
                    match decoded {
                        Some(envelope) => {
                            fjell_braille::render_lines(
                                &envelope,
                                fjell_braille::DEFAULT_WIDTH,
                                |line| {
                                    sys_debug_write("proxy-braille: ");
                                    sys_debug_writeln(line);
                                },
                            );
                        }
                        // Framing or decoding refused it: say so, as the text
                        // presentation's `ERR` reply does, rather than show
                        // nothing.
                        None => sys_debug_writeln("proxy-braille: envelope refused"),
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
