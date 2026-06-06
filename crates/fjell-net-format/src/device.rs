//! Network device descriptor — the capability-visible description of a
//! virtio-mmio network interface (RFC v0.4-001 §6.3).

/// Maximum transmission unit limit (driver-enforced).
pub const NET_MAX_MTU: u16 = 1514;
/// Minimum usable MTU (at least an Ethernet II header).
pub const NET_MIN_MTU: u16 = 64;

/// Opaque identifier for a registered network device.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NetDeviceId(pub u16);

impl NetDeviceId {
    pub const UNSET: Self = Self(0xFFFF);
    pub const fn is_unset(self) -> bool { self.0 == 0xFFFF }
}

/// 6-byte IEEE 802.3 MAC address.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NetMac(pub [u8; 6]);

impl NetMac {
    pub const ZERO: Self = Self([0; 6]);
}

/// Lifecycle state of a `NetDevice` capability object.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum NetDeviceState {
    /// The driver is negotiating features with the hardware.
    Initialising = 0x01,
    /// The device is ready; `netd` may exchange packets.
    Ready        = 0x02,
    /// A transient fault has been detected; restart may recover the device.
    Faulted      = 0x03,
    /// The capability has been explicitly revoked by `devmgr`.
    Revoked      = 0x04,
}

/// Capability-level descriptor for a `NetDevice` object.
///
/// This is the structure that `cap-broker` records when a `NetDevice`
/// capability is allocated.  It is read-only from the perspective of `netd`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NetDeviceDescriptor {
    pub device_id:   NetDeviceId,
    pub mac:         NetMac,
    pub mtu:         u16,
    pub state:       NetDeviceState,
    /// MMIO base address of the virtio-mmio device registers.
    pub mmio_base:   u64,
    /// Size of the MMIO region in bytes (typically 0x200 for virtio-mmio).
    pub mmio_size:   u64,
    /// IRQ line number at the PLIC.
    pub irq_line:    u16,
    /// DMA window base (RX ring).
    pub rx_dma_base: u64,
    /// DMA window base (TX ring).
    pub tx_dma_base: u64,
    /// DMA window size per ring (bytes; must be ≥ `NET_RING_SIZE_BYTES`).
    pub dma_size:    u64,
}

impl NetDeviceDescriptor {
    /// QEMU `virt` machine default net device parameters.
    pub const QEMU_VIRT_DEFAULT: Self = Self {
        device_id:   NetDeviceId(1),
        mac:         NetMac([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]),
        mtu:         1500,
        state:       NetDeviceState::Initialising,
        mmio_base:   0x1000_4000,
        mmio_size:   0x200,
        irq_line:    1,
        rx_dma_base: 0x8200_0000,
        tx_dma_base: 0x8201_0000,
        dma_size:    0x1000,
    };
}

/// Capability-level descriptor for an `Interrupt` capability object.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct InterruptDescriptor {
    /// PLIC interrupt source number (1-based, platform-specific).
    pub irq_line:      u16,
    /// PLIC priority for this line (1–7; 0 = disabled).
    pub plic_priority: u8,
    /// Whether the interrupt is currently enabled at the PLIC.
    pub enabled:       bool,
}

impl InterruptDescriptor {
    pub const QEMU_NET_DEFAULT: Self = Self {
        irq_line: 1, plic_priority: 1, enabled: false,
    };
}
