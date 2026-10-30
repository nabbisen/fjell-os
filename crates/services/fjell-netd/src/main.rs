//! netd — Packet and session routing service for Fjell OS.
//!
//! v0.4.0-alpha.1: Receives a `NetDevice` capability from cap-broker,
//! initialises the session table, and enters an event loop handling
//! `NET_LINK_UP` / `NET_PACKET_RX` from the driver (RFC v0.4-002).
//!
//! The session table and cap-broker integration land fully in v0.4.0-alpha.2.
// SMOKE-TEST STUB (v1.0 limitation; see docs/src/releasing/v1-limitations.md):
// this service signals readiness and exits before its main loop by design,
// so the kernel can emit the milestone marker. The full implementation is
// post-v1.0 roadmap work.
//
// RFC-0.37-002 (closes E-084): the event loop below used to live inside
// `service_main`, after its own `sys_exit(0)` -- statically unreachable,
// and hidden from the compiler's own `unreachable_code` lint by a
// crate-level `#![allow(...)]` that also suppressed five other lints
// across the whole file. It is real, finished code (`NET_LINK_UP`/
// `NET_PACKET_RX`/`NET_LINK_DOWN`/`NET_DEVICE_REVOKED` handling,
// pre-allocating a session for `secure-transportd`) and deleting it would
// be a real loss for whichever future line wires session/cap-broker
// integration in (this line's own non-goals forbid wiring it in here).
// Kept, as `run_event_loop` below: a free function `service_main` never
// calls, so a reader meets it as `dead_code` the compiler names
// explicitly ("function is never used") rather than as a code path buried
// after a diverging call in the function that runs.
#![no_std]
#![no_main]
mod rt;

use fjell_cap::CapHandle;
use fjell_net_format::{
    MAX_SESSIONS, NetDeviceDescriptor, NetIpcTag, NetSession, SessionError, SessionId, SessionState,
};
use fjell_syscall::{sys_debug_writeln, sys_exit};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    sys_debug_writeln("netd: panic");
    sys_exit(1);
}

// ── CSpace layout ─────────────────────────────────────────────────────────────
//
//   slot 0 — netd's own endpoint (`fjell_abi::service::NETD_EP_OBJECT`), the one
//            the event loop below would receive on. RFC-0.41-002: it used to be
//            described here as a NetDevice capability from cap-broker and, in
//            `recv_msg`, as "the netdev endpoint"; it was neither -- it was the
//            default endpoint every image without its own was handed, shared with
//            everything else. The NetDevice capability is not installed anywhere.
//
// RFC-0.28-001: the readiness signal no longer goes through a per-image
// CSpace slot at all. It used to declare "slot 2 — Endpoint to
// service-manager (ready signal)" here, but nothing in `spawn.rs` ever
// installed a capability there — the send silently failed every time
// (confirmed live: `sys_ipc_send` returned `InvalidCap`, discarded by the
// raw `asm!` call, so nothing ever printed). Every service now gets an
// unconditional, always-installed slot for this
// (`fjell_service_api::ready::SERVICE_READY_SEND_SLOT`) instead of a
// per-image declaration that has to be remembered to wire up.
//
// RFC-0.35-002 (closes E-074): slot 1 — "Endpoint to driver" — is gone.
// `recv_msg` below hardcodes slot 0, not this constant; nothing anywhere
// in the tree referenced it outside its own declaration (re-derived,
// RFC-0.35-001 R8). `service_main` exits before calling `run_event_loop`
// by design (the smoke-test stub this file's own top comment already
// discloses), so even that function never runs.
#[allow(dead_code)] // RFC-0.37-002 (closes E-082): re-derived, still true
const CAP_OWN_EP: CapHandle = CapHandle(0);

// ── IPC helpers ───────────────────────────────────────────────────────────────

fn send_ready() {
    // RFC-0.28-002: was a hand-rolled asm block; now the audited wrapper.
    let _ = fjell_syscall::sys_ipc_send(
        fjell_service_api::ready::SERVICE_READY_SEND_SLOT,
        fjell_service_api::tags::SERVICE_READY,
    );
}

fn recv_msg() -> (usize, usize, usize) {
    // RFC-0.28-002: was a hand-rolled `IpcRecv` asm block; `sys_ipc_recv_msg`
    // is a correct superset (`w2`/`w3`/sender discarded, not needed here).
    // Listen on this service's own endpoint (slot 0, `NETD_EP_OBJECT`).
    match fjell_syscall::sys_ipc_recv_msg(CAP_OWN_EP.0) {
        Ok((t, w0, w1, _w2, _w3, _sender)) => (t, w0, w1),
        Err(_) => (0, 0, 0),
    }
}

// ── Session table ─────────────────────────────────────────────────────────────

struct SessionTable {
    sessions: [NetSession; MAX_SESSIONS],
    count: u8,
}

impl SessionTable {
    // RFC-0.37-002 (closes E-082): `new` is only called from `run_event_loop`
    // below, itself never called (E-084) -- re-derived by building with the
    // crate-level blanket this line removed. One allow for the three
    // `SessionTable` methods only `run_event_loop` would ever call.
    #[allow(dead_code)]
    const fn new() -> Self {
        Self {
            sessions: [NetSession::EMPTY; MAX_SESSIONS],
            count: 0,
        }
    }

    fn alloc(&mut self, server_name: [u8; 64]) -> Result<SessionId, SessionError> {
        if self.count as usize >= MAX_SESSIONS {
            return Err(SessionError::SessionCapacityExhausted);
        }
        for slot in &mut self.sessions {
            if slot.state == SessionState::Closed {
                slot.session_id = SessionId(self.count as u16);
                slot.state = SessionState::Pending;
                slot.server_name = server_name;
                slot.channel_count = 0;
                self.count = self.count.wrapping_add(1);
                return Ok(slot.session_id);
            }
        }
        Err(SessionError::SessionCapacityExhausted)
    }

    // RFC-0.37-002 (closes E-082): not called by `run_event_loop` either --
    // a second, genuinely-unreached method, not inherited from the
    // crate-level blanket this line removed.
    #[allow(dead_code)]
    fn activate(&mut self, id: SessionId) -> bool {
        for slot in &mut self.sessions {
            if slot.session_id == id && slot.state == SessionState::Pending {
                slot.state = SessionState::Active;
                return true;
            }
        }
        false
    }

    #[allow(dead_code)]
    fn count_active(&self) -> usize {
        self.sessions
            .iter()
            .filter(|s| s.state == SessionState::Active)
            .count()
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("netd: starting");
    sys_debug_writeln("netd: session table initialised");
    send_ready();
    sys_debug_writeln("netd ready");
    // Smoke test: exit cleanly so kernel can emit TEST:V0.4-NET:PASS.
    sys_exit(0);
}

/// RFC-0.37-002 (closes E-084): the v0.4.0-alpha.1 event loop, written but
/// never called -- `service_main` exits before reaching it, by design, and
/// this line's own non-goals forbid wiring it in ("do not make `netd`
/// serve traffic here"). See this file's top comment for why it is kept
/// rather than deleted. `dev` is constructed here, not in `service_main`,
/// since nothing reads it before this loop would.
#[allow(dead_code)]
fn run_event_loop(mut sessions: SessionTable) -> ! {
    let _dev = NetDeviceDescriptor::QEMU_VIRT_DEFAULT;
    let mut link_up = false;

    loop {
        let (tag_raw, w0, _w1) = recv_msg();
        let tag = (tag_raw & 0xFFFF) as u16;

        match NetIpcTag::from_u16(tag) {
            Some(NetIpcTag::LinkUp) => {
                link_up = true;
                sys_debug_writeln("netd: link up");
                // Pre-allocate an update-metadata session for secure-transportd.
                let mut srv_name = [0u8; 64];
                srv_name[0] = b'u';
                srv_name[1] = b'p';
                srv_name[2] = b'd';
                let _ = sessions.alloc(srv_name);
            }
            Some(NetIpcTag::LinkDown) => {
                link_up = false;
                sys_debug_writeln("netd: link down");
            }
            Some(NetIpcTag::PacketRx) => {
                // In alpha.2 this will demux to the correct session.
                // For now, log and continue.
                let _ = (w0, link_up);
            }
            Some(NetIpcTag::DeviceRevoked) => {
                sys_debug_writeln("netd: NetDevice revoked; halting");
                sys_exit(1);
            }
            _ => {
                // Unknown tag; ignore.
            }
        }
    }
}
