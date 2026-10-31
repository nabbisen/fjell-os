//! Audit daemon (RFC 020).
//!
//! Drains the kernel audit ring via `sys_audit_drain` and emits each record
//! as a JSON Lines entry to the kernel UART.  In M7/M8 the log is
//! memory-backed only; persistent storage via `storaged` is M8 work.
//!
//! # How it is woken (RFC-0.41-001)
//! It drains **until the kernel reports the ring empty**, then parks in
//! `sys_audit_wait` until the next record is appended. It receives on **no
//! endpoint**: there is no sender, and none is needed. (It used to block on a
//! message "from other services (or init)" that nothing ever sent, so the trail
//! stopped at boot -- E-102.)
//!
//! # The trail has two kinds of line, told apart by shape
//! - a **record**, `{"seq":N,"kind":"label",...}`, is an audited event;
//! - a **gap notice**, `{"gap":{"dropped":N,"after_seq":S}}`, is not an event: it
//!   says the kernel's ring was full and dropped `N` records, which would have
//!   followed the record numbered `S`. It has no `seq` and no `kind`, so a reader
//!   cannot take it for an event, and a parser that expects a record fails on it
//!   rather than miscounting it.
//!
//! # Capability layout
//! - Slot 0: IPC endpoint (held by every service; **not used** by this one)
//! - Slot 1: `CapKind::AuditDrain` — required to call `sys_audit_drain` and
//!   `sys_audit_wait`

#![no_std]
#![no_main]
mod rt;

use fjell_audit_format::{AUDIT_RECORD_BIN_SIZE, AUDIT_RING_CAPACITY, AuditKind, AuditRecordBin};
use fjell_syscall::{
    sys_audit_drain, sys_audit_wait, sys_debug_write, sys_debug_writeln, sys_yield,
};

// ── Constants ─────────────────────────────────────────────────────────────────

/// CSpace slot holding the AuditDrain capability (granted by kernel on spawn).
const AUDIT_DRAIN_CAP: u32 = 1;

/// Drain buffer: up to 64 records × 32 bytes = 2048 bytes.
const DRAIN_BUF_RECORDS: usize = 64;
const DRAIN_BUF_BYTES: usize = DRAIN_BUF_RECORDS * AUDIT_RECORD_BIN_SIZE;

// ── JSON Lines formatting helpers ─────────────────────────────────────────────

/// Emit an unsigned 64-bit value as decimal ASCII.
fn emit_u64(mut n: u64) {
    if n == 0 {
        sys_debug_write("0");
        return;
    }
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    while n > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    if let Ok(s) = core::str::from_utf8(&buf[i..]) {
        sys_debug_write(s);
    }
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
    if res < 0 {
        sys_debug_write("-");
        emit_u64((-res) as u64);
    } else {
        emit_u64(res as u64);
    }
    sys_debug_writeln("}");
}

// ── Gap notices ───────────────────────────────────────────────────────────────

/// A loss the kernel reported: `dropped` records, which would have followed the
/// record numbered `after_seq`.
#[derive(Clone, Copy)]
struct Gap {
    dropped: u64,
    after_seq: u64,
}

/// Emit a gap notice. **Not an audited event**: it carries no `seq` and no
/// `kind`, only the `gap` key, so it cannot be mistaken for a record.
fn emit_gap(g: &Gap) {
    sys_debug_write(r#"{"gap":{"dropped":"#);
    emit_u64(g.dropped);
    sys_debug_write(r#","after_seq":"#);
    emit_u64(g.after_seq);
    sys_debug_writeln("}}");
}

// ── Drain until the kernel reports the ring empty ─────────────────────────────

/// Drain the ring until it is empty, printing every record and every loss.
///
/// One `sys_audit_drain` copies at most `DRAIN_BUF_RECORDS` (64) of up to 256
/// pending records, so a single drain per wake would leave up to 192 behind.
/// The loop, not the buffer, is what makes the trail complete.
///
/// **Where a loss goes.** The kernel reports a drop count with the first drain
/// after the ring overflowed. A ring that has dropped anything is full, and it
/// drops the *newest* record, so the lost records follow the ring's last pending
/// one: `AUDIT_RING_CAPACITY` records from the first one this batch returned. The
/// notice is therefore held until that record has been printed and then emitted
/// right after it. (A dropped record takes no sequence number, so the gap cannot
/// be read off the records' `seq` -- the next record continues the numbering.)
fn drain_until_empty(buf: &mut [u8; DRAIN_BUF_BYTES]) {
    let mut gap: Option<Gap> = None;
    loop {
        match sys_audit_drain(AUDIT_DRAIN_CAP, buf) {
            Ok((0, n_dropped)) => {
                // Empty. A loss reported with no records cannot be placed.
                if let Some(g) = gap.take() {
                    emit_gap(&g);
                }
                if n_dropped > 0 {
                    sys_debug_write(r#"{"gap":{"dropped":"#);
                    emit_u64(n_dropped as u64);
                    sys_debug_writeln("}}");
                }
                return;
            }
            Ok((n_records, n_dropped)) => {
                if n_dropped > 0 {
                    // A second overflow while the first is still waiting for its
                    // record: print the first now (early by at most one batch).
                    if let Some(g) = gap.take() {
                        emit_gap(&g);
                    }
                    if let Some(first) = AuditRecordBin::from_bytes(&buf[0..]) {
                        gap = Some(Gap {
                            dropped: n_dropped as u64,
                            after_seq: first.seq + AUDIT_RING_CAPACITY as u64 - 1,
                        });
                    }
                }
                for i in 0..n_records {
                    let off = i * AUDIT_RECORD_BIN_SIZE;
                    if let Some(rec) = AuditRecordBin::from_bytes(&buf[off..]) {
                        emit_record(&rec);
                        if let Some(g) = gap {
                            if rec.seq >= g.after_seq {
                                emit_gap(&g);
                                gap = None;
                            }
                        }
                    }
                }
            }
            Err(_) => {
                if let Some(g) = gap.take() {
                    emit_gap(&g);
                }
                sys_debug_writeln("auditd: drain error");
                return;
            }
        }
    }
}

// ── Service main ──────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("auditd: started");

    let mut buf = [0u8; DRAIN_BUF_BYTES];

    // Initial drain: capture everything the kernel logged during boot.
    drain_until_empty(&mut buf);

    // Park until the kernel appends a record, then drain until empty again. A record
    // appended between "the drain returned empty" and the wait makes the wait return
    // at once (the kernel checks the ring, it does not latch an event), so none is
    // slept through.
    loop {
        match sys_audit_wait(AUDIT_DRAIN_CAP) {
            Ok(true) => {}
            // The ring was already non-empty: a record arrived between the empty drain
            // and this wait, and the guard returned at once instead of sleeping through
            // it (RFC-0.41-001 R11). Said on the console so the `audit` tier can require
            // that the lost-wakeup sweep really reached this path.
            Ok(false) => sys_debug_writeln("auditd: wait did not park"),
            Err(_) => {
                // Cannot wait (e.g. another task already waits). Say so, and do not spin.
                sys_debug_writeln("auditd: audit wait error");
                sys_yield();
            }
        }
        drain_until_empty(&mut buf);
    }
}
