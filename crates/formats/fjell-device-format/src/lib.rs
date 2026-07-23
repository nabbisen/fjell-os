//! Device inventory and descriptor types for Fjell OS M6.
#![no_std]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceId(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind { VirtioMmioBlock, VirtioMmioNet, Uart, Timer, Unknown }

#[derive(Clone, Copy, Debug)]
pub struct MmioRegionDescriptor { pub base: u64, pub len: u64 }

#[derive(Clone, Copy, Debug)]
pub struct IrqDescriptor { pub irq: u32 }

#[derive(Clone, Copy, Debug)]
pub struct DeviceDescriptor {
    pub id:   DeviceId,
    pub kind: DeviceKind,
    pub mmio: Option<MmioRegionDescriptor>,
    pub irq:  Option<IrqDescriptor>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceState { Discovered, DriverMatched, DriverStarting, Ready, Faulted }

/// QEMU virt machine virtio-mmio block device.
pub const QEMU_VIRTIO_BLK: DeviceDescriptor = DeviceDescriptor {
    id:   DeviceId(0),
    kind: DeviceKind::VirtioMmioBlock,
    mmio: Some(MmioRegionDescriptor { base: 0x1000_1000, len: 0x1000 }),
    irq:  Some(IrqDescriptor { irq: 1 }),
};
