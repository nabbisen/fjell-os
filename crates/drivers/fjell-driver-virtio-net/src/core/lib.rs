//! Host-testable driver core for virtio-mmio network devices.
//!
//! This library is pure logic with no MMIO side-effects; it operates on
//! byte slices that the driver binary maps from real MMIO at runtime.
//!
//! RFC v0.4-001 §11.1 specifies the host-testable test targets.
#![no_std]

pub mod features;
pub mod mmio;
pub mod ring;
pub mod state;
pub mod virtq;

pub use features::{FeatureError, negotiate_features_checked};
pub use features::{
    VIRTIO_NET_F_MAC, VIRTIO_NET_F_MRG_RXBUF, VIRTIO_NET_F_STATUS, VirtioFeatureFlags,
    negotiate_features,
};
pub use mmio::{
    VIRTIO_INTR_CONFIG_CHANGE, VIRTIO_INTR_USED_BUFFER, VIRTIO_MMIO_MAGIC_VALUE,
    VIRTIO_MMIO_REGION_SIZE, VIRTIO_MMIO_STATUS, VIRTIO_NET_DEVICE_ID, VIRTIO_STATUS_DRIVER_OK,
    VIRTIO_STATUS_FAILED, init_status_sequence, read_le32, read_link_up, read_mac,
    verify_device_identity, write_le32,
};
pub use ring::{RING_SIZE, Ring, RingDescriptor, RingError, RingIndex, RingIndexCounter};
pub use state::{DriverState, DriverStateBlock, DriverStateError};
pub use virtq::{
    AvailRing, DescriptorAllocator, QUEUE_SIZE, UsedRing, VRING_DESC_F_NEXT, VRING_DESC_F_WRITE,
    VirtQueue, VirtqDesc,
};

#[cfg(test)]
mod tests;
