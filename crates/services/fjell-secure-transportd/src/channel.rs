//! Channel table and IPC tag definitions for `secure-transportd`.
//!
//! RFC v0.4-003 §5.2: SXT_* tag set.

use fjell_net_format::{ChannelKind, MAX_SXT_CHANNELS};

// ── IPC tag constants ─────────────────────────────────────────────────────────

// RFC-0.37-001 R2 (closes E-083): five of these twelve were also hand-copied
// by `fjell-diagnosticsd` under its own local `SXT_*` names — two
// independently-kept copies of one protocol. Moved to
// `fjell_service_api::sxt` as the single declaration both sides now read
// (the shape RFC-0.36-001 already used for `verifyd`'s tags); these five
// discriminants and match arms reference it rather than repeating the
// literal. The other seven have no second copy anywhere and keep their own
// literals.
const OPEN_CHANNEL: u16 = fjell_service_api::sxt::OPEN_CHANNEL as u16;
const OPENED: u16 = fjell_service_api::sxt::OPENED as u16;
const DIAG_PUSH: u16 = fjell_service_api::sxt::DIAG_PUSH as u16;
const DIAG_ACK: u16 = fjell_service_api::sxt::DIAG_ACK as u16;
const CLOSE: u16 = fjell_service_api::sxt::CLOSE as u16;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u16)]
pub enum SxtTag {
    OpenChannel = OPEN_CHANNEL,
    Opened = OPENED,
    UpdateMetadataFetch = 0x0102,
    UpdateMetadataReply = 0x0103,
    DiagPush = DIAG_PUSH,
    DiagAck = DIAG_ACK,
    AttestPush = 0x0106,
    AttestChallenge = 0x0107,
    FleetEnrollStep = 0x0108,
    Close = CLOSE,
    Closed = 0x010a,
    Faulted = 0x010b,
}

impl SxtTag {
    pub fn from_u16(v: u16) -> Option<Self> {
        match v {
            OPEN_CHANNEL => Some(Self::OpenChannel),
            OPENED => Some(Self::Opened),
            0x0102 => Some(Self::UpdateMetadataFetch),
            0x0103 => Some(Self::UpdateMetadataReply),
            DIAG_PUSH => Some(Self::DiagPush),
            DIAG_ACK => Some(Self::DiagAck),
            0x0106 => Some(Self::AttestPush),
            0x0107 => Some(Self::AttestChallenge),
            0x0108 => Some(Self::FleetEnrollStep),
            CLOSE => Some(Self::Close),
            0x010a => Some(Self::Closed),
            0x010b => Some(Self::Faulted),
            _ => None,
        }
    }
}

// ── Error codes ───────────────────────────────────────────────────────────────

/// RFC v0.4-003 §7.3 error codes.
///
/// RFC-0.37-002 (closes E-082): of these eleven, only `Internal` and
/// `HandshakeFailed` are returned in the default build today
/// (`CertVerifyFailed` only from the `simulated-transport`-gated dev path).
/// The rest are the full §7.3 code set, declared ahead of the paths that
/// will return them. One enum-level allow rather than nine per-variant
/// ones; re-derived, not inherited from the crate-level blanket this line
/// removed.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u16)]
pub enum SxtError {
    UnknownKind = 0x01,
    ServerNameNotPinned = 0x02,
    HandshakeFailed = 0x03,
    CertVerifyFailed = 0x04,
    HttpStrictReject = 0x05,
    ChannelClosed = 0x06,
    ChannelFaulted = 0x07,
    NoSessionCap = 0x08,
    SessionRevoked = 0x09,
    BodyTooLarge = 0x0A,
    Internal = 0xFFFF,
}

// ── Channel state ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum ChannelState {
    Free = 0x00,
    Negotiating = 0x01,
    Established = 0x02,
    // RFC-0.37-002 (closes E-082): declared for the full channel lifecycle
    // this type models; no code constructs either yet (open/close today
    // move straight between Negotiating/Established and Free). Re-derived,
    // not inherited from the crate-level blanket this line removed.
    #[allow(dead_code)]
    Draining = 0x03,
    #[allow(dead_code)]
    Faulted = 0x04,
}

/// Descriptor for one active channel (RFC v0.4-003 §5.1).
#[derive(Clone, Copy, Debug)]
pub struct ChannelDescriptor {
    pub channel_id: u32,
    pub kind: ChannelKind,
    pub server_name: [u8; 64],
    pub anchor_epoch: u32,
    pub state: ChannelState,
}

impl ChannelDescriptor {
    pub const EMPTY: Self = Self {
        channel_id: 0,
        kind: ChannelKind::UpdateMetadata,
        server_name: [0u8; 64],
        anchor_epoch: 0,
        state: ChannelState::Free,
    };
}

// ── Channel table ─────────────────────────────────────────────────────────────

/// RFC-0.35-001 R6: channel id `0` is invalid **by construction**, not by
/// the accident that `next_id` happens to start at `1`. Before this line
/// nothing read a real channel id at all (E-070), so the distinction never
/// mattered; now that `UpdateMetadataFetch`/`Close` address a channel by a
/// word a caller sends, a caller that sends `0` — the same `0` an
/// uninitialised or dropped word always was — must fail closed, not
/// accidentally succeed because no channel ever happens to hold it.
pub const INVALID_CHANNEL_ID: u32 = 0;

pub struct ChannelTable {
    channels: [ChannelDescriptor; MAX_SXT_CHANNELS],
    next_id: u32,
}

impl ChannelTable {
    pub const fn new() -> Self {
        Self {
            channels: [ChannelDescriptor::EMPTY; MAX_SXT_CHANNELS],
            next_id: 1,
        }
    }

    pub fn open(&mut self, kind: ChannelKind, server_name: [u8; 64]) -> Result<u32, SxtError> {
        for slot in &mut self.channels {
            if slot.state == ChannelState::Free {
                // `next_id` is advanced with `wrapping_add` (`u32::MAX`
                // opens would otherwise panic in a debug build rather than
                // fail closed); the one value it must never hand out is
                // `INVALID_CHANNEL_ID`, by construction, not by how far a
                // wrap is from happening.
                if self.next_id == INVALID_CHANNEL_ID {
                    self.next_id = self.next_id.wrapping_add(1);
                }
                slot.channel_id = self.next_id;
                slot.kind = kind;
                slot.server_name = server_name;
                slot.anchor_epoch = 0;
                slot.state = ChannelState::Negotiating;
                let id = self.next_id;
                self.next_id = self.next_id.wrapping_add(1);
                return Ok(id);
            }
        }
        Err(SxtError::Internal)
    }

    pub fn find_mut(&mut self, id: u32) -> Option<&mut ChannelDescriptor> {
        if id == INVALID_CHANNEL_ID {
            return None;
        }
        self.channels
            .iter_mut()
            .find(|c| c.channel_id == id && c.state != ChannelState::Free)
    }

    pub fn close(&mut self, id: u32) {
        if let Some(ch) = self.find_mut(id) {
            ch.state = ChannelState::Free;
        }
    }

    // RFC-0.37-002 (closes E-082): a diagnostic accessor nothing calls yet.
    // Re-derived, not inherited from the crate-level blanket this line
    // removed.
    #[allow(dead_code)]
    pub fn count_established(&self) -> usize {
        self.channels
            .iter()
            .filter(|c| c.state == ChannelState::Established)
            .count()
    }
}

// No host test module: this crate is a `[[bin]]`-only `#![no_std]
// #![no_main]` binary with its own `#[panic_handler]`, which collides with
// `std`'s under `cargo test` (`E0152: duplicate lang item panic_impl`) —
// confirmed live, not assumed. `ChannelTable`'s invariants are exercised by
// the `upgrade` QEMU tier instead (RFC-0.35-001 R7).
