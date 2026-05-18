//! Audit daemon (RFC 020).
//!
//! Drains the kernel audit ring via `sys_audit_drain` and emits each record
//! as a JSON Lines entry to the kernel UART.  In M7/M8 the log is
//! memory-backed only; persistent storage via `storaged` is M8 work.
//!
//! # Capability layout
//! - Slot 0: IPC endpoint (ep 0, shared)
//! - Slot 1: `CapKind::AuditDrain` — required to call `sys_audit_drain`

#![no_std]
#![no_main]
mod rt;

use fjell_audit_format::{AuditRecordBin, AuditKind, AUDIT_RECORD_BIN_SIZE};
use fjell_syscall::{sys_audit_drain, sys_ipc_recv, sys_debug_write, sys_debug_writeln};

// ── Constants ─────────────────────────────────────────────────────────────────

/// CSpace slot holding the AuditDrain capability (granted by kernel on spawn).
const AUDIT_DRAIN_CAP: u32 = 1;

/// Drain buffer: up to 64 records × 32 bytes = 2048 bytes.
const DRAIN_BUF_RECORDS: usize = 64;
const DRAIN_BUF_BYTES:   usize = DRAIN_BUF_RECORDS * AUDIT_RECORD_BIN_SIZE;

// ── JSON Lines formatting helpers ─────────────────────────────────────────────

/// Emit an unsigned 64-bit value as decimal ASCII.
fn emit_u64(mut n: u64) {
    if n == 0 { sys_debug_write("0"); return; }
    let mut buf = [0u8; 20];
    let mut i   = buf.len();
    while n > 0 { i -= 1; buf[i] = b'0' + (n % 10) as u8; n /= 10; }
    if let Ok(s) = core::str::from_utf8(&buf[i..]) { sys_debug_write(s); }
}

/// Emit one audit record as a JSON Lines object.
///
/// Format: `{"seq":N,"kind":"label","arg0":N,"arg1":N,"result":N}`
fn emit_record(r: &AuditRecordBin) {
    let kind = AuditKind::from_u16(r.kind);
    sys_debug_write(r#"{"seq":"#);
    emit_u64(r.seq);
    sys_debug_write(r#","kind":""#);
    sys_debug_write(kind.label());
    sys_debug_write(r#"","arg0":"#);
    emit_u64(r.arg0 as u64);
    sys_debug_write(r#","arg1":"#);
    emit_u64(r.arg1 as u64);
    sys_debug_write(r#","result":"#);
    // result is signed
    let res = r.result;
    if res < 0 { sys_debug_write("-"); emit_u64((-res) as u64); }
    else        { emit_u64(res as u64); }
    sys_debug_writeln("}");
}

// ── Drain one batch from the kernel ring ──────────────────────────────────────

fn drain_once(buf: &mut [u8; DRAIN_BUF_BYTES]) {
    match sys_audit_drain(buf, AUDIT_DRAIN_CAP) {
        Ok((n_records, _n_dropped)) => {
            for i in 0..n_records {
                let off = i * AUDIT_RECORD_BIN_SIZE;
                if let Some(rec) = AuditRecordBin::from_bytes(&buf[off..]) {
                    emit_record(&rec);
                }
            }
        }
        Err(_) => {
            sys_debug_writeln("auditd: drain error");
        }
    }
}

// ── Service main ──────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("auditd: started");

    let mut buf = [0u8; DRAIN_BUF_BYTES];

    // Initial drain: capture everything the kernel logged during boot.
    drain_once(&mut buf);

    // Event loop: wait for an IPC signal then drain again.
    // Other services (or init) send a message to endpoint 0 to trigger a drain.
    loop {
        // Block waiting for any IPC message on ep 0.
        let _ = sys_ipc_recv(0u32);
        drain_once(&mut buf);
    }
}
