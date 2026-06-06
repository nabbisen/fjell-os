//! Transport channel types for `secure-transportd` (RFC v0.4-003).
//!
//! `TransportChannel` describes one active TLS 1.3 channel between the
//! device and a pinned server endpoint.

use crate::session::ChannelKind;

/// Canonical table of `SXT_*` IPC channel-kind tags.
///
/// These are the wire tags used in IPC messages between `netd`/services and
/// `secure-transportd`.
pub const SXT_CHANNEL_KIND_TAGS: &[(ChannelKind, &str)] = &[
    (ChannelKind::UpdateMetadata, "SXT_RPC_UPDATE_METADATA"),
    (ChannelKind::Diagnostics,   "SXT_RPC_DIAG"),
    (ChannelKind::Attestation,   "SXT_RPC_ATTEST"),
    (ChannelKind::FleetEnroll,   "SXT_RPC_FLEET_ENROLL"),
];

/// Opaque identifier for a `TransportChannel`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ChannelId(pub u8);

impl ChannelId {
    pub const UNSET: Self = Self(0xFF);
}

/// Lifecycle state of a `TransportChannel`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum ChannelState {
    /// TLS handshake in progress.
    Handshaking  = 0x01,
    /// Channel is established and ready for RPC.
    Open         = 0x02,
    /// Application-level close initiated; draining.
    Closing      = 0x03,
    /// Channel has been closed normally.
    Closed       = 0x04,
    /// Channel closed due to a TLS or transport error.
    Faulted      = 0x05,
}

/// One active TLS 1.3 channel to a pinned server endpoint.
///
/// `secure-transportd` maintains up to `MAX_SXT_CHANNELS` of these.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TransportChannel {
    pub channel_id:    ChannelId,
    pub kind:          ChannelKind,
    pub state:         ChannelState,
    /// Pinned server SNI name (ASCII, zero-padded 64 B).
    pub server_name:   [u8; 64],
    /// SHA-256 fingerprint of the pinned server TLS leaf certificate.
    pub cert_pin:      [u8; 32],
    /// Total bytes sent on this channel.
    pub bytes_sent:    u64,
    /// Total bytes received on this channel.
    pub bytes_recv:    u64,
}

impl TransportChannel {
    pub const EMPTY: Self = Self {
        channel_id:  ChannelId::UNSET,
        kind:        ChannelKind::UpdateMetadata,
        state:       ChannelState::Closed,
        server_name: [0u8; 64],
        cert_pin:    [0u8; 32],
        bytes_sent:  0,
        bytes_recv:  0,
    };
}

/// Maximum simultaneous transport channels in `secure-transportd`.
pub const MAX_SXT_CHANNELS: usize = 4;
