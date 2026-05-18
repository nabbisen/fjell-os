//! User-space service SDK for Fjell OS.
//!
//! Provides helpers for IPC, debug output, and service protocol constants.

#![no_std]

pub mod tags {
    pub const SERVICE_READY:      usize = 0x001;
    pub const SERVICE_HEARTBEAT:  usize = 0x002;
    pub const SERVICE_SHUTDOWN:   usize = 0x003;
    pub const CONFIG_VALIDATE:    usize = 0x010;
    pub const CONFIG_VALIDATED:   usize = 0x011;
    pub const CONFIG_INVALID:     usize = 0x012;
    pub const CONFIG_GET:         usize = 0x013;
    pub const CAP_REQUEST:        usize = 0x020;
    pub const CAP_GRANTED:        usize = 0x021;
    pub const CAP_DENIED:         usize = 0x022;
    pub const AUDIT_EVENT:        usize = 0x030;
    pub const AUDIT_DRAIN_READY:  usize = 0x031;
    pub const SM_START_SERVICE:   usize = 0x040;
    pub const SM_STOP_SERVICE:    usize = 0x041;
    pub const SM_STATUS_QUERY:    usize = 0x042;
    pub const SM_STATUS_REPLY:    usize = 0x043;
    pub const SM_CORE_TARGET_READY: usize = 0x044;
    pub const BOOTSTRAP_COMPLETE: usize = 0x100;
}

// ── RFC 019: storaged IPC protocol ────────────────────────────────────────────
pub mod storaged {
    /// Storaged is ready; init may proceed with storage operations.
    pub const READY:            usize = 0x200;
    /// Begin a 512-byte sector write. words[1]=lba_lo, words[2]=lba_hi.
    pub const WRITE_BEGIN:      usize = 0x201;
    /// One 64-byte chunk of sector data. words[0..8] = data bytes (little-endian).
    pub const WRITE_CHUNK:      usize = 0x202;
    /// Commit the staged write. Reply: WRITE_OK or WRITE_ERR.
    pub const WRITE_COMMIT:     usize = 0x203;
    pub const WRITE_ACK:        usize = 0x204;  // ack for BEGIN/CHUNK
    pub const WRITE_OK:         usize = 0x205;
    pub const WRITE_ERR:        usize = 0x206;
}

// ── RFC 019: bootctl IPC protocol ─────────────────────────────────────────────
pub mod bootctl {
    pub const READY:            usize = 0x210;
    /// Read the BCB; reply is READ_OK with 8-chunk transfer, then BCB_DATA.
    pub const READ_BCB:         usize = 0x211;
    /// Write the BCB; follow with 8 WRITE_CHUNK messages then WRITE_COMMIT.
    pub const WRITE_BCB:        usize = 0x212;
    pub const READ_OK:          usize = 0x213;
    pub const WRITE_OK:         usize = 0x214;
    pub const ERR:              usize = 0x215;
}
