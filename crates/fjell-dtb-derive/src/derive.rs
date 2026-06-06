//! Derive a `BoardProfile` from a DTB byte slice (RFC v0.5-002 §7.2).

use fjell_platform_format::{
    PlatformProfile, BoardProfile, BoardDevice, DeviceClass, RecoveryDescriptor,
    RecoveryKind, platform_digest, board_digest,
};
use fjell_measure_format::Digest32;
use crate::parser::{parse_header, FdtIter, NodeEvent, FdtProp, ParseError};
use crate::compat::{CompatString, ALLOWED_COMPAT_QEMU_VIRT};

/// Context passed to the derivation function.
pub struct DeriveContext {
    pub platform:        PlatformProfile,
    pub dma_base:        u64,
    pub dma_total:       u64,
    pub dma_per_device:  u64,
    pub allowed_compat:  &'static [CompatString],
}

impl DeriveContext {
    /// Default context for the QEMU `virt` machine.
    pub fn qemu_virt_default(platform: PlatformProfile) -> Self {
        Self {
            platform,
            dma_base:       0x8000_0000,
            dma_total:      0x4000_0000, // 1 GiB
            dma_per_device: 0x0400_0000, // 64 MiB each
            allowed_compat: ALLOWED_COMPAT_QEMU_VIRT,
        }
    }
}

/// Errors from profile derivation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeriveError {
    DtbParseFailed(ParseError),
    TooManyDevices,
    MissingPlic,
    OverlappingRanges,
    OutOfDmaSpace,
}

impl From<ParseError> for DeriveError {
    fn from(e: ParseError) -> Self { Self::DtbParseFailed(e) }
}

/// Derive a fully-populated `BoardProfile` from a raw DTB byte slice.
///
/// Steps (per RFC v0.5-002 §7.3):
/// 1. Parse and validate the DTB header.
/// 2. Walk all device nodes; classify by `compatible` strings.
/// 3. Assign DMA windows from the `DeriveContext` pool.
/// 4. Compute and stamp `platform_digest` and `board_digest`.
pub fn derive_board_profile(
    dtb:     &[u8],
    ctx:     &DeriveContext,
    name:    &[u8; 16],
    rev:     &[u8; 8],
) -> Result<BoardProfile, DeriveError> {
    let hdr = parse_header(dtb)?;

    let mut devices: [BoardDevice; 16] = [BoardDevice::EMPTY; 16];
    let mut count = 0usize;
    let mut found_plic = false;
    let mut dma_cursor = ctx.dma_base;

    let iter = FdtIter::new(dtb, &hdr);
    let mut node_depth = 0usize;
    let mut cur_mmio: u64 = 0;
    let mut cur_size: u64 = 0;
    let mut cur_irq:  u16 = 0;
    let mut cur_class: Option<DeviceClass> = None;
    let mut cur_name = [0u8; 16];

    for event in iter {
        match event.map_err(DeriveError::DtbParseFailed)? {
            NodeEvent::BeginNode { name } => {
                node_depth += 1;
                cur_mmio = 0; cur_size = 0; cur_irq = 0;
                cur_class = None;
                let n = name.len().min(15);
                cur_name = [0u8; 16];
                cur_name[..n].copy_from_slice(&name[..n]);
            }
            NodeEvent::EndNode => {
                if node_depth == 2 {
                    if let Some(class) = cur_class {
                        if count >= 16 { return Err(DeriveError::TooManyDevices); }
                        let needs_dma = matches!(class,
                            DeviceClass::VirtioNetMmio | DeviceClass::VirtioBlkMmio);
                        let (dma_start, dma_size) = if needs_dma {
                            if dma_cursor + ctx.dma_per_device > ctx.dma_base + ctx.dma_total {
                                return Err(DeriveError::OutOfDmaSpace);
                            }
                            let s = dma_cursor;
                            dma_cursor += ctx.dma_per_device;
                            (s, ctx.dma_per_device)
                        } else {
                            (0, 0)
                        };
                        if class == DeviceClass::Plic { found_plic = true; }
                        devices[count] = BoardDevice {
                            class,
                            mmio_base: cur_mmio,
                            mmio_size: cur_size,
                            irq_line:  cur_irq,
                            dma_window_start: dma_start,
                            dma_window_size:  dma_size,
                            name: cur_name,
                        };
                        count += 1;
                    }
                }
                node_depth = node_depth.saturating_sub(1);
            }
            NodeEvent::Prop(FdtProp { name: prop, value }) => {
                if node_depth < 2 { continue; }
                if prop == b"compatible" {
                    cur_class = classify_compat(value);
                } else if prop == b"reg" && value.len() >= 16 {
                    // reg = <addr_hi addr_lo size_hi size_lo> (2-cell addressing)
                    cur_mmio = u64::from_be_bytes([
                        value[0],value[1],value[2],value[3],
                        value[4],value[5],value[6],value[7],
                    ]);
                    cur_size = u64::from_be_bytes([
                        value[8],value[9],value[10],value[11],
                        value[12],value[13],value[14],value[15],
                    ]);
                } else if prop == b"interrupts" && value.len() >= 4 {
                    cur_irq = u32::from_be_bytes([
                        value[0],value[1],value[2],value[3]
                    ]) as u16;
                }
            }
        }
    }

    if !found_plic { return Err(DeriveError::MissingPlic); }

    // Stamp platform digest.
    let mut pp = ctx.platform;
    pp.profile_digest = platform_digest(&pp);

    let mut bp = BoardProfile {
        schema_version: fjell_platform_format::BOARD_PROFILE_VERSION,
        board_name:     *name,
        board_revision: *rev,
        platform_ref:   pp.profile_digest,
        device_count:   count as u8,
        devices,
        recovery: RecoveryDescriptor {
            kind: RecoveryKind::BootArg,
            mmio_base: 0,
            gpio_pin:  0,
        },
        profile_digest: Digest32([0u8; 32]),
    };
    bp.profile_digest = board_digest(&bp);
    Ok(bp)
}

fn classify_compat(value: &[u8]) -> Option<DeviceClass> {
    for part in value.split(|&b| b == 0) {
        if part.is_empty() { continue; }
        if part == b"ns16550a"   { return Some(DeviceClass::Uart8250); }
        if part == b"virtio,mmio"{ return Some(DeviceClass::VirtioNetMmio); }
        if part == b"riscv,plic0"{ return Some(DeviceClass::Plic); }
        if part == b"riscv,clint0"{ return Some(DeviceClass::Clint); }
    }
    None
}
