//! Session and channel types for `netd` (RFC v0.4-002 §6.1).
//!
//! A `NetSession` groups a bounded set of `ChannelKind`-typed transport
//! channels allocated by `netd` for each connected upper-layer consumer.

/// Maximum concurrent sessions per `netd` instance.
pub const MAX_SESSIONS: usize = 8;
/// Maximum open channels per session.
pub const MAX_CHANNELS: usize = 4;

/// Opaque session identifier.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SessionId(pub u16);

impl SessionId {
    pub const UNSET: Self = Self(0xFFFF);
}

/// Lifecycle state of a `NetSession`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum SessionState {
    /// Session has been allocated but not yet verified.
    Pending    = 0x01,
    /// Session is active; channels may be opened.
    Active     = 0x02,
    /// All channels have been closed; session is draining.
    Draining   = 0x03,
    /// Session has been closed and the slot is free.
    Closed     = 0x04,
    /// Session was closed due to a transport or identity error.
    Faulted    = 0x05,
}

/// Kind of transport channel opened within a session.
///
/// Used both as a routing discriminant in `netd` and as the
/// `ChannelKind` field in `secure-transportd` (RFC v0.4-003 §5.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum ChannelKind {
    /// Update-metadata fetch (RFC v0.4-004).
    UpdateMetadata  = 0x01,
    /// Diagnostic bundle push (RFC v0.4-005).
    Diagnostics     = 0x02,
    /// Attestation record push / challenge (RFC v0.4-005).
    Attestation     = 0x03,
    /// Fleet enrollment (RFC v0.8-001 forward-compat reservation).
    FleetEnroll     = 0x04,
}

impl ChannelKind {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::UpdateMetadata),
            0x02 => Some(Self::Diagnostics),
            0x03 => Some(Self::Attestation),
            0x04 => Some(Self::FleetEnroll),
            _    => None,
        }
    }

    pub fn tag(self) -> u8 { self as u8 }
}

/// An active network session — one per upper-layer consumer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NetSession {
    pub session_id:     SessionId,
    pub state:          SessionState,
    /// Peer server name (SNI-equivalent; ASCII, zero-padded).
    pub server_name:    [u8; 64],
    pub channel_count:  u8,
    pub channels:       [Option<ChannelId>; MAX_CHANNELS],
}

impl NetSession {
    pub const EMPTY: Self = Self {
        session_id:    SessionId::UNSET,
        state:         SessionState::Closed,
        server_name:   [0u8; 64],
        channel_count: 0,
        channels:      [None; MAX_CHANNELS],
    };
}

/// Opaque channel identifier within a session.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ChannelId(pub u8);

/// Errors from session / channel operations.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum SessionError {
    /// No free session slots (`MAX_SESSIONS` reached).
    SessionCapacityExhausted  = 0x01,
    /// No free channel slots within the session (`MAX_CHANNELS` reached).
    ChannelCapacityExhausted  = 0x02,
    /// The referenced session ID is not active.
    SessionNotFound           = 0x03,
    /// The referenced channel ID is not open.
    ChannelNotFound           = 0x04,
    /// Operation rejected — session is not in the required state.
    InvalidState              = 0x05,
    /// Unknown `ChannelKind` discriminant.
    UnknownChannelKind        = 0x06,
}
