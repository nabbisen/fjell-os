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
