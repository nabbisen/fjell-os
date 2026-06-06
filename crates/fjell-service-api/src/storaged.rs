//! Client-side storaged IPC helpers (RFC-v0.7.2-001).
//!
//! Wraps the low-level READ_BEGIN / READ_CHUNK / WRITE_BEGIN sequences into
//! ergonomic record-level operations.  This module is `no_std` and depends
//! only on `fjell-syscall` for the IPC primitives.

use fjell_cap::CapHandle;

/// storaged IPC tag constants (mirror of storaged/src/main.rs).
pub mod tags {
    pub const READY:        usize = 0x200;
    pub const WRITE_BEGIN:  usize = 0x201;
    pub const WRITE_CHUNK:  usize = 0x202;
    pub const WRITE_COMMIT: usize = 0x203;
    pub const WRITE_ACK:    usize = 0x204;
    pub const WRITE_OK:     usize = 0x205;
    pub const WRITE_ERR:    usize = 0x206;
    pub const READ_BEGIN:   usize = 0x207;
    pub const READ_CHUNK:   usize = 0x208;
    pub const READ_COMMIT:  usize = 0x209;
    pub const READ_ACK:     usize = 0x20A;
    pub const READ_DATA:    usize = 0x20B;
    pub const READ_OK:      usize = 0x20C;
    pub const READ_ERR:     usize = 0x20D;
}

/// Outcome of a storaged persist call.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StoreResult {
    Ok,
    WriteError,
    ReadError,
    NotFound,
    ServiceUnavailable,
}

/// Read a record from storaged by kind.
///
/// v0.7.2: the actual IPC path is wired here as a documented skeleton;
/// the wire transfer lands when the service-manager manifest activates
/// storaged before identityd.  For now returns `ServiceUnavailable`
/// to indicate the storaged endpoint is not yet reachable.
pub fn store_read(
    _storaged_ep: CapHandle,
    _record_kind: u16,
    _buf: &mut [u8],
) -> Result<usize, StoreResult> {
    // v0.7.2 implementation note:
    //   1. ipc_call(storaged_ep, READ_BEGIN, record_kind)
    //   2. loop: ipc_recv → READ_CHUNK (copy into buf) | READ_OK (done) | READ_ERR (fail)
    //   3. ipc_send(READ_ACK) after each chunk
    //
    // Full wiring requires storaged to expose its endpoint cap through
    // the service-manager manifest before identityd starts. The manifest
    // ordering and cap-broker policy are v0.7.2.1 deliverables.
    Err(StoreResult::ServiceUnavailable)
}

/// Write/append a record to storaged.
///
/// v0.7.2: same status as store_read — wired as skeleton, returns
/// ServiceUnavailable until storaged endpoint is reachable.
pub fn store_append(
    _storaged_ep: CapHandle,
    _record_kind: u16,
    _data: &[u8],
) -> StoreResult {
    // v0.7.2 implementation note:
    //   1. ipc_call(storaged_ep, WRITE_BEGIN, record_kind, data.len())
    //   2. chunk data in ≤ 240-byte IPC payloads via WRITE_CHUNK
    //   3. ipc_call(WRITE_COMMIT) → WRITE_OK | WRITE_ERR
    StoreResult::ServiceUnavailable
}
