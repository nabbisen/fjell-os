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

pub use mmio::{
    read_le32, write_le32, read_mac, read_link_up,
    verify_device_identity, init_status_sequence,
    VIRTIO_MMIO_MAGIC_VALUE, VIRTIO_NET_DEVICE_ID,
    VIRTIO_STATUS_DRIVER_OK, VIRTIO_STATUS_FAILED,
    VIRTIO_MMIO_REGION_SIZE, VIRTIO_MMIO_STATUS,
    VIRTIO_INTR_USED_BUFFER, VIRTIO_INTR_CONFIG_CHANGE,
};
pub use features::{
    VirtioFeatureFlags, negotiate_features,
    VIRTIO_NET_F_MAC, VIRTIO_NET_F_STATUS, VIRTIO_NET_F_MRG_RXBUF,
};
pub use ring::{
    RingIndex, RingIndexCounter, RingDescriptor, Ring,
    RING_SIZE, RingError,
};
pub use state::{DriverState, DriverStateBlock, DriverStateError};

#[cfg(test)]
mod tests;
