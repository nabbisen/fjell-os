//! Virtio-net feature flag negotiation (RFC v0.4-001 §7.1).
//!
//! v0.4.0 supports a minimal feature set; all unknown features are masked out.

/// Bit-flag set for virtio feature negotiation.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct VirtioFeatureFlags(pub u64);

impl VirtioFeatureFlags {
    pub fn contains(self, bit: u64) -> bool { (self.0 & bit) != 0 }
    pub fn with(self, bit: u64) -> Self     { Self(self.0 | bit) }
    pub fn without(self, bit: u64) -> Self  { Self(self.0 & !bit) }
}

// ── Feature bit constants (virtio-net spec 1.2, §5.1.3) ──────────────────────

/// Device provides a MAC address (should always be set on virtio-mmio).
pub const VIRTIO_NET_F_MAC:        u64 = 1 << 5;
/// Device provides link status via the config space.
pub const VIRTIO_NET_F_STATUS:     u64 = 1 << 16;
/// Merge RX buffers (not supported in v0.4; always masked out).
pub const VIRTIO_NET_F_MRG_RXBUF:  u64 = 1 << 15;
/// Checksum offload (not supported in v0.4; always masked out).
pub const VIRTIO_F_RING_INDIRECT:  u64 = 1 << 28;
/// Event suppression (not used; masked out for simplicity).
pub const VIRTIO_F_EVENT_IDX:      u64 = 1 << 29;

/// Feature bits accepted by this driver (v0.4.0 minimal set).
pub const DRIVER_ACCEPTED_FEATURES: u64 =
    VIRTIO_NET_F_MAC | VIRTIO_NET_F_STATUS;

/// Negotiate features: return the intersection of device-offered and
/// driver-accepted features, masking out everything else.
///
/// Returns `(negotiated, legacy_mode)`.
/// `legacy_mode` is `true` when the device does not offer
/// `VIRTIO_F_VERSION_1` (bit 32), meaning the driver must use the
/// legacy register layout.
pub fn negotiate_features(device_offered: VirtioFeatureFlags) -> (VirtioFeatureFlags, bool) {
    const VIRTIO_F_VERSION_1: u64 = 1 << 32;
    let legacy = !device_offered.contains(VIRTIO_F_VERSION_1);
    let negotiated = VirtioFeatureFlags(device_offered.0 & DRIVER_ACCEPTED_FEATURES);
    (negotiated, legacy)
}
