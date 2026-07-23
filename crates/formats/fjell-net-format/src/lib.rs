//! Network format types for Fjell OS.
//!
//! Defines the capability-visible objects and protocol surfaces for the
//! virtio-net driver (`driver-virtio-net`), the packet/session service
//! (`netd`), and the authenticated control-plane channel (`secure-transportd`).
//!
//! RFCs v0.4-001 (device capabilities), v0.4-002 (sessions), v0.4-003
//! (transport channel kinds).
#![no_std]

pub mod device;
pub mod session;
pub mod channel;
pub mod proto;

pub use device::{
    NetDeviceDescriptor, NetDeviceId, NetDeviceState,
    InterruptDescriptor, NetMac, NET_MAX_MTU, NET_MIN_MTU,
};
pub use session::{
    ChannelKind, NetSession, SessionId, SessionState,
    MAX_SESSIONS, MAX_CHANNELS, SessionError,
};
pub use channel::{
    TransportChannel, ChannelId, ChannelState,
    SXT_CHANNEL_KIND_TAGS, MAX_SXT_CHANNELS,
};
pub use proto::{
    NetIpcTag, NetDriverPacket, NET_RING_DESCRIPTORS, NET_RING_SIZE_BYTES,
    NET_DESCRIPTOR_PAYLOAD,
};

#[cfg(test)]
mod tests;
