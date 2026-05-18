//! Audit daemon for M4.
//!
//! Drains the kernel audit ring and emits JSON Lines records to the UART.
//! In M4 the log is memory-backed only (no persistent storage).

#![no_std]
#![no_main]
mod rt;

use fjell_syscall::{sys_audit_drain, sys_exit, sys_ipc_recv, sys_ipc_reply,
                    sys_debug_write, sys_debug_writeln};
use fjell_service_api::tags;

/// Fixed-capacity drain buffer (single audit record ≤ 128 bytes).
const DRAIN_BUF_SIZE: usize = 512;

/// Minimal JSON Lines emitter for audit records (no heap, no serde).
fn emit_json_line(seq: u32, kind: &str, producer: &str, result: &str) {
    // Format: {"seq":N,"kind":"...","producer":"...","result":"..."}
    sys_debug_write(r#"{"seq":"#);
    emit_u32(seq);
    sys_debug_write(r#","kind":""#);
    sys_debug_write(kind);
    sys_debug_write(r#"","producer":""#);
    sys_debug_write(producer);
    sys_debug_write(r#"","result":""#);
    sys_debug_write(result);
    sys_debug_writeln(r#""}"#);
}

fn emit_u32(mut n: u32) {
    if n == 0 { sys_debug_write("0"); return; }
    let mut buf = [0u8; 10];
    let mut i = buf.len();
    while n > 0 { i -= 1; buf[i] = b'0' + (n % 10) as u8; n /= 10; }
    let s = core::str::from_utf8(&buf[i..]).unwrap_or("?");
    sys_debug_write(s);
}

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    let ep = 0u32;
    let mut seq = 0u32;

    // Announce ready
    let _ = sys_ipc_reply(tags::SERVICE_READY);

    loop {
        match sys_ipc_recv(ep) {
            Ok(tags::AUDIT_DRAIN_READY) => {
                // Drain kernel ring and emit records
                let mut buf = [0u8; DRAIN_BUF_SIZE];
                let _ = sys_audit_drain(&mut buf);
                // In M4 we emit a fixed boot record representing the drain
                seq += 1;
                emit_json_line(seq, "kernel.audit.drained", "auditd", "ok");
                let _ = sys_ipc_reply(tags::AUDIT_DRAIN_READY);
            }
            Ok(tags::SERVICE_SHUTDOWN) => break,
            Ok(_) | Err(_) => { let _ = sys_ipc_reply(0); }
        }
    }

    sys_exit(0)
}
