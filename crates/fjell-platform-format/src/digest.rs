//! Canonical SHA-256 digest computation (RFC v0.5-001 §6.3).

use fjell_measure_format::Digest32;
use crate::platform::{PlatformProfile, PLATFORM_PROFILE_VERSION};
use crate::board::{BoardProfile, BOARD_PROFILE_VERSION};

/// Compute the canonical `platform_digest` over a `PlatformProfile`.
///
/// Layout (RFC v0.5-001 §6.3):
/// ```text
/// SHA256("FJELL-PLATFORM-V1" ||
///        schema_version u16 LE || family u8 || family_version u16 LE ||
///        isa_extensions u64 LE ||
///        kernel_abi (major u8 || minor u8) ||
///        mem_map (6 × u64 LE) ||
///        plic_layout (base u64 || size u64 || sources u16 || contexts u16))
/// ```
pub fn platform_digest(p: &PlatformProfile) -> Digest32 {
    let mut buf = [0u8; 256];
    let mut pos = 0;

    macro_rules! w_u8  { ($v:expr) => { buf[pos] = $v; pos += 1; }; }
    macro_rules! w_u16 { ($v:expr) => { buf[pos..pos+2].copy_from_slice(&($v as u16).to_le_bytes()); pos += 2; }; }
    macro_rules! w_u64 { ($v:expr) => { buf[pos..pos+8].copy_from_slice(&($v as u64).to_le_bytes()); pos += 8; }; }
    macro_rules! w_bytes { ($b:expr) => { let b: &[u8] = $b; buf[pos..pos+b.len()].copy_from_slice(b); pos += b.len(); }; }

    w_bytes!(b"FJELL-PLATFORM-V1");
    w_u16!(PLATFORM_PROFILE_VERSION);
    w_u8! (p.family as u8);
    w_u16!(p.family_version);
    w_u64!(p.isa_extensions.0);
    w_u8! (p.kernel_abi.major);
    w_u8! (p.kernel_abi.minor);
    // MemMap — 6 × u64
    w_u64!(p.mem_map.kernel_load_addr);
    w_u64!(p.mem_map.kernel_size_max);
    w_u64!(p.mem_map.heap_start);
    w_u64!(p.mem_map.heap_size);
    w_u64!(p.mem_map.initrd_addr);
    w_u64!(p.mem_map.initrd_size);
    // PlicLayout
    w_u64!(p.plic_layout.base_addr);
    w_u64!(p.plic_layout.size_bytes);
    w_u16!(p.plic_layout.num_sources);
    w_u16!(p.plic_layout.num_contexts);

    Digest32::of(&buf[..pos])
}

/// Compute the canonical `board_digest` over a `BoardProfile`.
///
/// Layout (RFC v0.5-001 §6.3):
/// ```text
/// SHA256("FJELL-BOARD-V1" ||
///        schema_version u16 LE || board_name 16B || board_revision 8B ||
///        platform_ref 32B || device_count u8 ||
///        for each device: class u8 || mmio_base u64 LE || mmio_size u64 LE ||
///                         irq_line u16 LE || dma_start u64 LE || dma_size u64 LE ||
///                         name 16B ||
///        recovery_kind u8 || recovery_mmio u64 LE || gpio_pin u16 LE)
/// ```
pub fn board_digest(b_: &BoardProfile) -> Digest32 {
    // Max size: 14 + 2 + 16 + 8 + 32 + 1 + 16*(1+8+8+2+8+8+16) + 11 = ~900 B.
    let mut buf = [0u8; 1024];
    let mut pos = 0;

    macro_rules! w_u8  { ($v:expr) => { buf[pos] = $v; pos += 1; }; }
    macro_rules! w_u16 { ($v:expr) => { buf[pos..pos+2].copy_from_slice(&($v as u16).to_le_bytes()); pos += 2; }; }
    macro_rules! w_u64 { ($v:expr) => { buf[pos..pos+8].copy_from_slice(&($v as u64).to_le_bytes()); pos += 8; }; }
    macro_rules! w_bytes { ($b:expr) => { let bb: &[u8] = $b; buf[pos..pos+bb.len()].copy_from_slice(bb); pos += bb.len(); }; }

    w_bytes!(b"FJELL-BOARD-V1");
    w_u16!(BOARD_PROFILE_VERSION);
    w_bytes!(&b_.board_name);
    w_bytes!(&b_.board_revision);
    w_bytes!(&b_.platform_ref.0);
    w_u8! (b_.device_count);
    for i in 0..b_.device_count as usize {
        let d = &b_.devices[i];
        w_u8! (d.class as u8);
        w_u64!(d.mmio_base);
        w_u64!(d.mmio_size);
        w_u16!(d.irq_line);
        w_u64!(d.dma_window_start);
        w_u64!(d.dma_window_size);
        w_bytes!(&d.name);
    }
    w_u8! (b_.recovery.kind as u8);
    w_u64!(b_.recovery.mmio_base);
    w_u16!(b_.recovery.gpio_pin);

    Digest32::of(&buf[..pos])
}
