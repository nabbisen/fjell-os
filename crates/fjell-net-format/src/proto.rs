//! IPC protocol tags and virtio-mmio ring constants (RFC v0.4-001 §5.2).

// ── virtio-mmio ring dimensions ──────────────────────────────────────────────

/// Number of descriptors in one RX or TX ring.
pub const NET_RING_DESCRIPTORS: usize = 16;
/// Size of one ring in bytes (one DMA page).
pub const NET_RING_SIZE_BYTES:  usize = 4096;
/// Maximum payload in a single ring descriptor.
pub const NET_DESCRIPTOR_PAYLOAD: usize = 240;

// ── IPC tag constants (driver-virtio-net ↔ netd) ──────────────────────────────

/// IPC tag constants used between `driver-virtio-net` and `netd`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u16)]
pub enum NetIpcTag {
    /// Driver → netd: a packet has been received into the RX ring.
    PacketRx       = 0x0010,
    /// netd → driver: place a packet into the TX ring.
    PacketTx       = 0x0011,
    /// Driver → netd: a TX ring slot has been freed.
    TxDone         = 0x0012,
    /// Driver → netd: the link has come up.
    LinkUp         = 0x0013,
    /// Driver → netd: the link has gone down.
    LinkDown       = 0x0014,
    /// Driver → netd: the `NetDevice` capability has been revoked.
    DeviceRevoked  = 0x0015,
    /// netd → driver: query current link + queue state.
    QueryState     = 0x0016,
    /// Driver → netd: reply to a `QueryState` message.
    QueryReply     = 0x0017,
    /// Driver → service-manager: driver initialisation complete.
    DriverReady    = 0x0018,
}

impl NetIpcTag {
    pub fn from_u16(v: u16) -> Option<Self> {
        match v {
            0x0010 => Some(Self::PacketRx),
            0x0011 => Some(Self::PacketTx),
            0x0012 => Some(Self::TxDone),
            0x0013 => Some(Self::LinkUp),
            0x0014 => Some(Self::LinkDown),
            0x0015 => Some(Self::DeviceRevoked),
            0x0016 => Some(Self::QueryState),
            0x0017 => Some(Self::QueryReply),
            0x0018 => Some(Self::DriverReady),
            _      => None,
        }
    }
}

// ── Ring descriptor header (in-memory, shared DMA) ───────────────────────────

/// Fixed-width header at the front of every DMA ring descriptor slot.
///
/// Layout: 4 B header + up to `NET_DESCRIPTOR_PAYLOAD` B payload.
/// The `len` field is the number of valid payload bytes (0 for an empty slot).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(C)]
pub struct NetDescriptorHeader {
    /// Valid payload length in bytes (LE).
    pub len:   u16,
    /// Protocol flags; driver-private in v0.4 (MBZ for external consumers).
    pub flags: u16,
}

/// IPC-level packet reference (ring index, not pointer) carried by
/// `PacketRx` / `PacketTx` messages.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NetDriverPacket {
    /// Index into the DMA ring (0 to `NET_RING_DESCRIPTORS - 1`).
    pub ring_idx: u16,
    /// Byte length of the packet data in the descriptor.
    pub pkt_len:  u16,
    /// Protocol flags (driver-private; MBZ for external consumers).
    pub flags:    u32,
}

impl NetDriverPacket {
    pub fn is_valid(&self) -> bool {
        (self.ring_idx as usize) < NET_RING_DESCRIPTORS
            && self.pkt_len as usize <= NET_DESCRIPTOR_PAYLOAD
    }
}
