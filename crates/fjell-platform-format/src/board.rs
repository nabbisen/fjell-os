//! `BoardProfile` — concrete board variant descriptor (RFC v0.5-001 §6.2).

use fjell_measure_format::Digest32;

pub const BOARD_PROFILE_VERSION: u16 = 1;
pub const MAX_BOARD_DEVICES:     usize = 16;

/// Functional class of a board device.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum DeviceClass {
    Uart8250       = 0x01,
    VirtioNetMmio  = 0x02,
    VirtioBlkMmio  = 0x03,
    VirtioConsole  = 0x04,
    Plic           = 0x05,
    Clint          = 0x06,
    SystemCounter  = 0x07,
    Generic        = 0xFF,
}

impl DeviceClass {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::Uart8250),
            0x02 => Some(Self::VirtioNetMmio),
            0x03 => Some(Self::VirtioBlkMmio),
            0x04 => Some(Self::VirtioConsole),
            0x05 => Some(Self::Plic),
            0x06 => Some(Self::Clint),
            0x07 => Some(Self::SystemCounter),
            0xFF => Some(Self::Generic),
            _    => None,
        }
    }
}

/// A single hardware device entry in the board profile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoardDevice {
    pub class:             DeviceClass,
    pub mmio_base:         u64,
    pub mmio_size:         u64,
    pub irq_line:          u16,
    pub dma_window_start:  u64,
    pub dma_window_size:   u64,
    pub name:              [u8; 16],  // ASCII, zero-padded
}

impl BoardDevice {
    pub const EMPTY: Self = Self {
        class:            DeviceClass::Generic,
        mmio_base:        0,
        mmio_size:        0,
        irq_line:         0,
        dma_window_start: 0,
        dma_window_size:  0,
        name:             [0u8; 16],
    };
}

/// Recovery trigger mechanism.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum RecoveryKind {
    None        = 0,
    BootArg     = 1,   // kernel boot arg "recovery=1"
    Gpio        = 2,
    SerialBreak = 3,
}

/// Board recovery entry-point descriptor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RecoveryDescriptor {
    pub kind:      RecoveryKind,
    pub mmio_base: u64,  // 0 if not MMIO
    pub gpio_pin:  u16,  // 0 if not GPIO
}

/// Top-level board descriptor.
///
/// `profile_digest` must be computed by `crate::digest::board_digest` and
/// written into this field before the profile is stored or measured.
#[derive(Clone, Copy, Debug)]
pub struct BoardProfile {
    pub schema_version:  u16,
    pub board_name:      [u8; 16],   // ASCII, zero-padded
    pub board_revision:  [u8; 8],
    /// SHA-256 of the associated `PlatformProfile`.
    pub platform_ref:    Digest32,
    pub device_count:    u8,
    pub devices:         [BoardDevice; MAX_BOARD_DEVICES],
    pub recovery:        RecoveryDescriptor,
    pub profile_digest:  Digest32,
}

impl BoardProfile {
    /// QEMU `virt` board profile matching the CI test setup.
    ///
    /// Devices match the QEMU `virt` memory map at default offsets.
    /// `profile_digest` is zeroed; call `board_digest` and write back.
    pub fn qemu_virt_default(platform_ref: Digest32) -> Self {
        let mut devices = [BoardDevice::EMPTY; MAX_BOARD_DEVICES];
        let mut name_buf = |s: &[u8]| -> [u8; 16] {
            let mut b = [0u8; 16];
            let n = s.len().min(16);
            b[..n].copy_from_slice(&s[..n]);
            b
        };
        devices[0] = BoardDevice {
            class:            DeviceClass::Uart8250,
            mmio_base:        0x1000_0000,
            mmio_size:        0x1000,
            irq_line:         10,
            dma_window_start: 0,
            dma_window_size:  0,
            name:             name_buf(b"uart0"),
        };
        devices[1] = BoardDevice {
            class:            DeviceClass::VirtioNetMmio,
            mmio_base:        0x1000_1000,
            mmio_size:        0x1000,
            irq_line:         1,
            dma_window_start: 0x8000_0000,
            dma_window_size:  0x4000_0000,
            name:             name_buf(b"virtio-net0"),
        };
        devices[2] = BoardDevice {
            class:            DeviceClass::VirtioBlkMmio,
            mmio_base:        0x1000_2000,
            mmio_size:        0x1000,
            irq_line:         2,
            dma_window_start: 0x8000_0000,
            dma_window_size:  0x4000_0000,
            name:             name_buf(b"virtio-blk0"),
        };
        devices[3] = BoardDevice {
            class:            DeviceClass::Plic,
            mmio_base:        0x0C00_0000,
            mmio_size:        0x0400_0000,
            irq_line:         0,
            dma_window_start: 0,
            dma_window_size:  0,
            name:             name_buf(b"plic0"),
        };
        devices[4] = BoardDevice {
            class:            DeviceClass::Clint,
            mmio_base:        0x0200_0000,
            mmio_size:        0x0001_0000,
            irq_line:         0,
            dma_window_start: 0,
            dma_window_size:  0,
            name:             name_buf(b"clint0"),
        };

        let mut bn = [0u8; 16];
        bn[..10].copy_from_slice(b"qemu-virt0");
        let mut br = [0u8; 8];
        br[..3].copy_from_slice(b"v05");

        Self {
            schema_version:  BOARD_PROFILE_VERSION,
            board_name:      bn,
            board_revision:  br,
            platform_ref,
            device_count:    5,
            devices,
            recovery: RecoveryDescriptor {
                kind:      RecoveryKind::BootArg,
                mmio_base: 0,
                gpio_pin:  0,
            },
            profile_digest:  Digest32([0u8; 32]),
        }
    }
}
