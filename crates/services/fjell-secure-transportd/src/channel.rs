//! Channel table and IPC tag definitions for `secure-transportd`.
//!
//! RFC v0.4-003 §5.2: SXT_* tag set.

use fjell_net_format::{ChannelKind, MAX_SXT_CHANNELS};

// ── IPC tag constants ─────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u16)]
pub enum SxtTag {
    OpenChannel          = 0x0100,
    Opened               = 0x0101,
    UpdateMetadataFetch  = 0x0102,
    UpdateMetadataReply  = 0x0103,
    DiagPush             = 0x0104,
    DiagAck              = 0x0105,
    AttestPush           = 0x0106,
    AttestChallenge      = 0x0107,
    FleetEnrollStep      = 0x0108,
    Close                = 0x0109,
    Closed               = 0x010a,
    Faulted              = 0x010b,
}

impl SxtTag {
    pub fn from_u16(v: u16) -> Option<Self> {
        match v {
            0x0100 => Some(Self::OpenChannel),
            0x0101 => Some(Self::Opened),
            0x0102 => Some(Self::UpdateMetadataFetch),
            0x0103 => Some(Self::UpdateMetadataReply),
            0x0104 => Some(Self::DiagPush),
            0x0105 => Some(Self::DiagAck),
            0x0106 => Some(Self::AttestPush),
            0x0107 => Some(Self::AttestChallenge),
            0x0108 => Some(Self::FleetEnrollStep),
            0x0109 => Some(Self::Close),
            0x010a => Some(Self::Closed),
            0x010b => Some(Self::Faulted),
            _ => None,
        }
    }
}

// ── Error codes ───────────────────────────────────────────────────────────────

/// RFC v0.4-003 §7.3 error codes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u16)]
pub enum SxtError {
    UnknownKind         = 0x01,
    ServerNameNotPinned = 0x02,
    HandshakeFailed     = 0x03,
    CertVerifyFailed    = 0x04,
    HttpStrictReject    = 0x05,
    ChannelClosed       = 0x06,
    ChannelFaulted      = 0x07,
    NoSessionCap        = 0x08,
    SessionRevoked      = 0x09,
    BodyTooLarge        = 0x0A,
    Internal            = 0xFFFF,
}

// ── Channel state ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum ChannelState {
    Free        = 0x00,
    Negotiating = 0x01,
    Established = 0x02,
    Draining    = 0x03,
    Faulted     = 0x04,
}

/// Descriptor for one active channel (RFC v0.4-003 §5.1).
#[derive(Clone, Copy, Debug)]
pub struct ChannelDescriptor {
    pub channel_id:   u32,
    pub kind:         ChannelKind,
    pub server_name:  [u8; 64],
    pub anchor_epoch: u32,
    pub state:        ChannelState,
}

impl ChannelDescriptor {
    pub const EMPTY: Self = Self {
        channel_id:   0,
        kind:         ChannelKind::UpdateMetadata,
        server_name:  [0u8; 64],
        anchor_epoch: 0,
        state:        ChannelState::Free,
    };
}

// ── Channel table ─────────────────────────────────────────────────────────────

pub struct ChannelTable {
    channels: [ChannelDescriptor; MAX_SXT_CHANNELS],
    next_id:  u32,
}

impl ChannelTable {
    pub const fn new() -> Self {
        Self {
            channels: [ChannelDescriptor::EMPTY; MAX_SXT_CHANNELS],
            next_id:  1,
        }
    }

    pub fn open(
        &mut self,
        kind:        ChannelKind,
        server_name: [u8; 64],
    ) -> Result<u32, SxtError> {
        for slot in &mut self.channels {
            if slot.state == ChannelState::Free {
                slot.channel_id  = self.next_id;
                slot.kind        = kind;
                slot.server_name = server_name;
                slot.anchor_epoch = 0;
                slot.state       = ChannelState::Negotiating;
                let id = self.next_id;
                self.next_id = self.next_id.wrapping_add(1);
                return Ok(id);
            }
        }
        Err(SxtError::Internal)
    }

    pub fn find_mut(&mut self, id: u32) -> Option<&mut ChannelDescriptor> {
        self.channels.iter_mut().find(|c| c.channel_id == id && c.state != ChannelState::Free)
    }

    pub fn close(&mut self, id: u32) {
        if let Some(ch) = self.find_mut(id) {
            ch.state = ChannelState::Free;
        }
    }

    pub fn count_established(&self) -> usize {
        self.channels.iter().filter(|c| c.state == ChannelState::Established).count()
    }
}
