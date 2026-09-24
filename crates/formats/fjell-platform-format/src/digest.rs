//! Canonical SHA-256 digest computation (RFC v0.5-001 §6.3).

use crate::board::{BOARD_PROFILE_VERSION, BoardProfile};
use crate::platform::{PLATFORM_PROFILE_VERSION, PlatformProfile};
use fjell_canon::{BufSink, Canon};
use fjell_measure_format::Digest32;

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
    let mut sink = BufSink::<256>::new();
    write_platform_canonical(p, &mut sink);
    Digest32::of(sink.bytes())
}

/// The canonical byte stream `platform_digest` is taken over — **the function
/// the digest is computed from and the frozen schema is generated from**.
pub fn write_platform_canonical(p: &PlatformProfile, c: &mut dyn Canon) {
    c.domain(b"FJELL-PLATFORM-V1");
    c.u16("schema_version", PLATFORM_PROFILE_VERSION);
    c.u8("family", p.family as u8);
    c.u16("family_version", p.family_version);
    c.u64("isa_extensions", p.isa_extensions.0);
    c.u8("kernel_abi.major", p.kernel_abi.major);
    c.u8("kernel_abi.minor", p.kernel_abi.minor);
    // MemMap — 6 × u64
    c.u64("mem_map.kernel_load_addr", p.mem_map.kernel_load_addr);
    c.u64("mem_map.kernel_size_max", p.mem_map.kernel_size_max);
    c.u64("mem_map.heap_start", p.mem_map.heap_start);
    c.u64("mem_map.heap_size", p.mem_map.heap_size);
    c.u64("mem_map.initrd_addr", p.mem_map.initrd_addr);
    c.u64("mem_map.initrd_size", p.mem_map.initrd_size);
    // PlicLayout
    c.u64("plic_layout.base_addr", p.plic_layout.base_addr);
    c.u64("plic_layout.size_bytes", p.plic_layout.size_bytes);
    c.u16("plic_layout.num_sources", p.plic_layout.num_sources);
    c.u16("plic_layout.num_contexts", p.plic_layout.num_contexts);
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
    let mut sink = BufSink::<1024>::new();
    write_board_canonical(b_, &mut sink);
    Digest32::of(sink.bytes())
}

/// The canonical byte stream `board_digest` is taken over — **the function the
/// digest is computed from and the frozen schema is generated from**.
pub fn write_board_canonical(b_: &BoardProfile, c: &mut dyn Canon) {
    c.domain(b"FJELL-BOARD-V1");
    c.u16("schema_version", BOARD_PROFILE_VERSION);
    c.bytes("board_name", &b_.board_name);
    c.bytes("board_revision", &b_.board_revision);
    c.bytes("platform_ref", &b_.platform_ref.0);
    c.u8("device_count", b_.device_count);
    c.each(
        "devices",
        b_.device_count as usize,
        Some(16),
        &mut |c, i| {
            let d = &b_.devices[i];
            c.u8("class", d.class as u8);
            c.u64("mmio_base", d.mmio_base);
            c.u64("mmio_size", d.mmio_size);
            c.u16("irq_line", d.irq_line);
            c.u64("dma_window_start", d.dma_window_start);
            c.u64("dma_window_size", d.dma_window_size);
            c.bytes("name", &d.name);
        },
    );
    c.u8("recovery.kind", b_.recovery.kind as u8);
    c.u64("recovery.mmio_base", b_.recovery.mmio_base);
    c.u16("recovery.gpio_pin", b_.recovery.gpio_pin);
}
