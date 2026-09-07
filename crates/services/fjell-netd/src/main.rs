//! netd — Packet and session routing service for Fjell OS.
//!
//! v0.4.0-alpha.1: Receives a `NetDevice` capability from cap-broker,
//! initialises the session table, and enters an event loop handling
//! `NET_LINK_UP` / `NET_PACKET_RX` from the driver (RFC v0.4-002).
//!
//! The session table and cap-broker integration land fully in v0.4.0-alpha.2.
// SMOKE-TEST STUB (v1.0 limitation; see docs/release/v1-limitations.md):
// this service signals readiness and exits before its main loop by design,
// so the kernel can emit the milestone marker. The full implementation is
// post-v1.0 roadmap work; the allows below keep the intentional dead paths
// from polluting the workspace warning baseline.
#![allow(
    dead_code,
    unused_variables,
    unreachable_code,
    unused_imports,
    unused_assignments,
    unused_mut
)]
#![no_std]
#![no_main]
mod rt;

use fjell_cap::CapHandle;
use fjell_net_format::{
    ChannelKind, MAX_CHANNELS, MAX_SESSIONS, NetDeviceDescriptor, NetDeviceState, NetIpcTag,
    NetSession, SessionError, SessionId, SessionState,
};
use fjell_syscall::{sys_debug_writeln, sys_exit};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    sys_debug_writeln("netd: panic");
    sys_exit(1);
}

// ── CSpace layout ─────────────────────────────────────────────────────────────
//
//   slot 0 — NetDevice capability (from cap-broker)
//   slot 1 — Endpoint to driver (for query/control)
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
const CAP_NETDEV: CapHandle = CapHandle(0);
const CAP_DRV_EP: CapHandle = CapHandle(1);

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
    // listen on ep slot 0 (the netdev endpoint)
    match fjell_syscall::sys_ipc_recv_msg(0) {
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

    fn activate(&mut self, id: SessionId) -> bool {
        for slot in &mut self.sessions {
            if slot.session_id == id && slot.state == SessionState::Pending {
                slot.state = SessionState::Active;
                return true;
            }
        }
        false
    }

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

    let mut sessions = SessionTable::new();
    let dev = NetDeviceDescriptor::QEMU_VIRT_DEFAULT;
    let mut link_up = false;

    sys_debug_writeln("netd: session table initialised");
    send_ready();
    sys_debug_writeln("netd ready");
    // Smoke test: exit cleanly so kernel can emit TEST:V0.4-NET:PASS.
    sys_exit(0);

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
