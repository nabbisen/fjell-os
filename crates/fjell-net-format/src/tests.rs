//! Host unit tests for `fjell-net-format` (RFC v0.4-001 §11.1, RFC v0.4-002 §11.1).

use crate::device::{
    InterruptDescriptor, NetDeviceDescriptor, NetDeviceId, NetDeviceState,
    NetMac, NET_MAX_MTU, NET_MIN_MTU,
};
use crate::session::{
    ChannelId, ChannelKind, MAX_CHANNELS, MAX_SESSIONS,
    NetSession, SessionError, SessionId, SessionState,
};
use crate::channel::{ChannelState, MAX_SXT_CHANNELS, TransportChannel};
use crate::proto::{
    NetDriverPacket, NetIpcTag, NET_RING_DESCRIPTORS, NET_RING_SIZE_BYTES,
    NET_DESCRIPTOR_PAYLOAD,
};

// ── Device constants ─────────────────────────────────────────────────────────

#[test]
fn net_mtu_constants_sane() {
    assert!(NET_MIN_MTU < NET_MAX_MTU);
    assert_eq!(NET_MAX_MTU, 1514);
    assert_eq!(NET_MIN_MTU, 64);
}

#[test]
fn net_device_id_unset_sentinel() {
    assert!(NetDeviceId::UNSET.is_unset());
    assert!(!NetDeviceId(1).is_unset());
}

#[test]
fn net_mac_zero_is_all_zeros() {
    assert_eq!(NetMac::ZERO.0, [0u8; 6]);
}

#[test]
fn net_device_state_repr_stable() {
    assert_eq!(NetDeviceState::Initialising as u8, 0x01);
    assert_eq!(NetDeviceState::Ready        as u8, 0x02);
    assert_eq!(NetDeviceState::Faulted      as u8, 0x03);
    assert_eq!(NetDeviceState::Revoked      as u8, 0x04);
}

#[test]
fn net_device_descriptor_qemu_default_sensible() {
    let d = NetDeviceDescriptor::QEMU_VIRT_DEFAULT;
    assert!(!d.device_id.is_unset());
    assert!(d.mtu >= NET_MIN_MTU && d.mtu <= NET_MAX_MTU);
    assert_eq!(d.irq_line, 1);
    assert!(d.mmio_size >= 0x200);
}

#[test]
fn interrupt_descriptor_qemu_default_sensible() {
    let i = InterruptDescriptor::QEMU_NET_DEFAULT;
    assert_eq!(i.irq_line, 1);
    assert!(i.plic_priority >= 1);
}

// ── Session constants ────────────────────────────────────────────────────────

#[test]
fn session_capacity_constants() {
    assert_eq!(MAX_SESSIONS, 8);
    assert_eq!(MAX_CHANNELS, 4);
}

#[test]
fn channel_kind_round_trip() {
    for (tag, kind) in [
        (0x01u8, ChannelKind::UpdateMetadata),
        (0x02,   ChannelKind::Diagnostics),
        (0x03,   ChannelKind::Attestation),
        (0x04,   ChannelKind::FleetEnroll),
    ] {
        assert_eq!(ChannelKind::from_u8(tag), Some(kind));
        assert_eq!(kind.tag(), tag);
    }
    assert_eq!(ChannelKind::from_u8(0xFF), None);
}

#[test]
fn session_state_repr_stable() {
    assert_eq!(SessionState::Pending  as u8, 0x01);
    assert_eq!(SessionState::Active   as u8, 0x02);
    assert_eq!(SessionState::Draining as u8, 0x03);
    assert_eq!(SessionState::Closed   as u8, 0x04);
    assert_eq!(SessionState::Faulted  as u8, 0x05);
}

#[test]
fn session_empty_is_closed() {
    let s = NetSession::EMPTY;
    assert_eq!(s.session_id, SessionId::UNSET);
    assert_eq!(s.state, SessionState::Closed);
    assert_eq!(s.channel_count, 0);
}

#[test]
fn session_error_codes_stable() {
    assert_eq!(SessionError::SessionCapacityExhausted as u8, 0x01);
    assert_eq!(SessionError::ChannelCapacityExhausted as u8, 0x02);
    assert_eq!(SessionError::UnknownChannelKind       as u8, 0x06);
}

// ── Transport channel ────────────────────────────────────────────────────────

#[test]
fn max_sxt_channels_is_four() {
    assert_eq!(MAX_SXT_CHANNELS, 4);
}

#[test]
fn channel_state_repr_stable() {
    assert_eq!(ChannelState::Handshaking as u8, 0x01);
    assert_eq!(ChannelState::Open        as u8, 0x02);
    assert_eq!(ChannelState::Closed      as u8, 0x04);
    assert_eq!(ChannelState::Faulted     as u8, 0x05);
}

#[test]
fn transport_channel_empty_is_closed() {
    let c = TransportChannel::EMPTY;
    assert_eq!(c.state, ChannelState::Closed);
    assert_eq!(c.bytes_sent, 0);
    assert_eq!(c.bytes_recv, 0);
}

// ── Ring and IPC protocol ────────────────────────────────────────────────────

#[test]
fn ring_constants_consistent() {
    assert_eq!(NET_RING_SIZE_BYTES, 4096);
    assert_eq!(NET_RING_DESCRIPTORS, 16);
    // Each descriptor must fit payload + header (4 B) within the ring.
    assert!(NET_RING_DESCRIPTORS * (NET_DESCRIPTOR_PAYLOAD + 4) <= NET_RING_SIZE_BYTES);
}

#[test]
fn net_ipc_tag_round_trip() {
    for (raw, tag) in [
        (0x0010u16, NetIpcTag::PacketRx),
        (0x0011,    NetIpcTag::PacketTx),
        (0x0012,    NetIpcTag::TxDone),
        (0x0013,    NetIpcTag::LinkUp),
        (0x0014,    NetIpcTag::LinkDown),
        (0x0015,    NetIpcTag::DeviceRevoked),
        (0x0018,    NetIpcTag::DriverReady),
    ] {
        assert_eq!(NetIpcTag::from_u16(raw), Some(tag));
        assert_eq!(tag as u16, raw);
    }
    assert_eq!(NetIpcTag::from_u16(0xFFFF), None);
}

#[test]
fn net_driver_packet_valid_within_ring() {
    let p = NetDriverPacket { ring_idx: 0, pkt_len: 64, flags: 0 };
    assert!(p.is_valid());
}

#[test]
fn net_driver_packet_invalid_ring_idx() {
    let p = NetDriverPacket { ring_idx: NET_RING_DESCRIPTORS as u16, pkt_len: 1, flags: 0 };
    assert!(!p.is_valid());
}

#[test]
fn net_driver_packet_invalid_oversized() {
    let p = NetDriverPacket {
        ring_idx: 0,
        pkt_len: (NET_DESCRIPTOR_PAYLOAD + 1) as u16,
        flags: 0,
    };
    assert!(!p.is_valid());
}
