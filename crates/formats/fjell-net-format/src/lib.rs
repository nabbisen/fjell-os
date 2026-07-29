//! Network format types for Fjell OS.
//!
//! Defines the capability-visible objects and protocol surfaces for the
//! virtio-net driver (`driver-virtio-net`), the packet/session service
//! (`netd`), and the authenticated control-plane channel (`secure-transportd`).
//!
//! RFCs v0.4-001 (device capabilities), v0.4-002 (sessions), v0.4-003
//! (transport channel kinds).
#![no_std]

pub mod channel;
pub mod device;
pub mod proto;
pub mod session;

pub use channel::{
    ChannelId, ChannelState, MAX_SXT_CHANNELS, SXT_CHANNEL_KIND_TAGS, TransportChannel,
};
pub use device::{
    InterruptDescriptor, NET_MAX_MTU, NET_MIN_MTU, NetDeviceDescriptor, NetDeviceId,
    NetDeviceState, NetMac,
};
pub use proto::{
    NET_DESCRIPTOR_PAYLOAD, NET_RING_DESCRIPTORS, NET_RING_SIZE_BYTES, NetDriverPacket, NetIpcTag,
};
pub use session::{
    ChannelKind, MAX_CHANNELS, MAX_SESSIONS, NetSession, SessionError, SessionId, SessionState,
};

#[cfg(test)]
mod tests;
