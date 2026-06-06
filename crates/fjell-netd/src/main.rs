//! netd — Packet and session routing service for Fjell OS.
//!
//! v0.4.0-alpha.1: Receives a `NetDevice` capability from cap-broker,
//! initialises the session table, and enters an event loop handling
//! `NET_LINK_UP` / `NET_PACKET_RX` from the driver (RFC v0.4-002).
//!
//! The session table and cap-broker integration land fully in v0.4.0-alpha.2.
#![no_std]
#![no_main]
mod rt;

use fjell_syscall::{sys_debug_writeln, sys_exit};
use fjell_cap::CapHandle;
use fjell_net_format::{
    NetSession, SessionId, SessionState, ChannelKind,
    MAX_SESSIONS, MAX_CHANNELS, SessionError,
    NetIpcTag, NetDeviceDescriptor, NetDeviceState,
};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    sys_debug_writeln("netd: panic");
    sys_exit(1);
}

// ── CSpace layout ─────────────────────────────────────────────────────────────
//
//   slot 0 — NetDevice capability (from cap-broker)
//   slot 1 — Endpoint to driver (for query/control)
//   slot 2 — Endpoint to service-manager (ready signal)
//
const CAP_NETDEV:  CapHandle = CapHandle(0);
const CAP_DRV_EP:  CapHandle = CapHandle(1);
const CAP_SMGR_EP: CapHandle = CapHandle(2);

// ── IPC helpers ───────────────────────────────────────────────────────────────

const READY_TAG: usize = 0x0001;

fn send_ready(ep: CapHandle) {
    unsafe {
        core::arch::asm!(
            "li a7, 20", "ecall",
            in("a0") ep.0 as usize, in("a1") READY_TAG,
            lateout("a0") _, lateout("a7") _, options(nostack)
        );
    }
}

fn recv_msg() -> (usize, usize, usize) {
    let (mut t, mut w0, mut w1) = (0usize, 0usize, 0usize);
    unsafe {
        core::arch::asm!(
            "li a7, 21", "ecall",
            in("a0") 0usize, // listen on ep slot 0 (the netdev endpoint)
            lateout("a1") t, lateout("a2") w0, lateout("a3") w1,
            lateout("a4") _, lateout("a5") _, lateout("a7") _, options(nostack)
        );
    }
    (t, w0, w1)
}

// ── Session table ─────────────────────────────────────────────────────────────

struct SessionTable {
    sessions: [NetSession; MAX_SESSIONS],
    count:    u8,
}

impl SessionTable {
    const fn new() -> Self {
        Self {
            sessions: [NetSession::EMPTY; MAX_SESSIONS],
            count:    0,
        }
    }

    fn alloc(&mut self, server_name: [u8; 64]) -> Result<SessionId, SessionError> {
        if self.count as usize >= MAX_SESSIONS {
            return Err(SessionError::SessionCapacityExhausted);
        }
        for slot in &mut self.sessions {
            if slot.state == SessionState::Closed {
                slot.session_id  = SessionId(self.count as u16);
                slot.state       = SessionState::Pending;
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
        self.sessions.iter().filter(|s| s.state == SessionState::Active).count()
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
    send_ready(CAP_SMGR_EP);
    sys_debug_writeln("netd ready");

    loop {
        let (tag_raw, w0, _w1) = recv_msg();
        let tag = (tag_raw & 0xFFFF) as u16;

        match NetIpcTag::from_u16(tag) {
            Some(NetIpcTag::LinkUp) => {
                link_up = true;
                sys_debug_writeln("netd: link up");
                // Pre-allocate an update-metadata session for secure-transportd.
                let mut srv_name = [0u8; 64];
                srv_name[0] = b'u'; srv_name[1] = b'p'; srv_name[2] = b'd';
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
