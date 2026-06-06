//! Host unit tests for `fjell-dtb-derive` (RFC v0.5-002 §11).
//!
//! Uses hand-built minimal DTB byte slices to avoid shipping external .dtb
//! files in the source tree.

use crate::parser::{parse_header, FdtIter, NodeEvent, ParseError,
                    FDT_MAGIC, FDT_VERSION,
                    FDT_BEGIN_NODE, FDT_END_NODE, FDT_PROP, FDT_END};
use crate::compat::{matches_compat, ALLOWED_COMPAT_QEMU_VIRT};
use crate::derive::{derive_board_profile, DeriveContext, DeriveError};
use fjell_platform_format::{PlatformProfile, DeviceClass};

// ── Minimal DTB builder ───────────────────────────────────────────────────────

/// Minimal DTB builder.
/// Layout: [header 0x28 B][struct block][strings block(1 NUL)]
struct DtbBuilder {
    buf: [u8; 4096],
    pos: usize,  // current write position (starts after header)
}

impl DtbBuilder {
    fn new() -> Self {
        let mut s = Self { buf: [0u8; 4096], pos: 0x28 }; // leave room for header
        s
    }
    fn be32(&mut self, v: u32) {
        self.buf[self.pos..self.pos+4].copy_from_slice(&v.to_be_bytes());
        self.pos += 4;
    }
    fn bytes(&mut self, b: &[u8]) {
        self.buf[self.pos..self.pos+b.len()].copy_from_slice(b);
        self.pos += b.len();
    }
    fn align4(&mut self) {
        while self.pos % 4 != 0 { self.buf[self.pos] = 0; self.pos += 1; }
    }
    fn cstr(&mut self, s: &[u8]) {
        self.bytes(s);
        self.buf[self.pos] = 0; self.pos += 1;
        self.align4();
    }
    fn begin_node(&mut self, name: &[u8]) { self.be32(FDT_BEGIN_NODE); self.cstr(name); }
    fn end_node(&mut self) { self.be32(FDT_END_NODE); }
    fn prop(&mut self, name_off: u32, value: &[u8]) {
        self.be32(FDT_PROP);
        self.be32(value.len() as u32);
        self.be32(name_off);
        self.bytes(value);
        self.align4();
    }
    fn fdt_end(&mut self) { self.be32(FDT_END); }

    /// Finalise: stamp the 0x28-byte FDT header at offset 0.
    fn finish(mut self) -> [u8; 4096] {
        // Append the strings block with all property names used in tests.
        //   offset 0: "compatible\0"
        //   offset 11: "reg\0"
        //   offset 15: "interrupts\0"
        let off_strings = self.pos as u32;
        for name in [b"compatible\0" as &[u8], b"reg\0", b"interrupts\0"] {
            self.buf[self.pos..self.pos+name.len()].copy_from_slice(name);
            self.pos += name.len();
        }
        let total = self.pos as u32;

        let off_struct: u32 = 0x28;
        let size_str: u32   = self.pos as u32 - off_strings;
        let size_struct: u32 = off_strings - off_struct;

        let mut h = [0u8; 0x28];
        h[0..4] .copy_from_slice(&FDT_MAGIC.to_be_bytes());
        h[4..8] .copy_from_slice(&total.to_be_bytes());
        h[8..12].copy_from_slice(&off_struct.to_be_bytes());
        h[12..16].copy_from_slice(&off_strings.to_be_bytes());
        h[16..20].copy_from_slice(&0u32.to_be_bytes());   // mem_rsvmap
        h[20..24].copy_from_slice(&FDT_VERSION.to_be_bytes());
        h[24..28].copy_from_slice(&FDT_VERSION.to_be_bytes());
        h[28..32].copy_from_slice(&0u32.to_be_bytes());   // cpuid
        h[32..36].copy_from_slice(&size_str.to_be_bytes());
        h[36..40].copy_from_slice(&size_struct.to_be_bytes());
        self.buf[..0x28].copy_from_slice(&h);
        self.buf
    }
}

/// Build a minimal valid DTB with a root node and FDT_END.
fn minimal_dtb() -> [u8; 4096] {
    let mut b = DtbBuilder::new();
    b.begin_node(b"");
    b.end_node();
    b.fdt_end();
    b.finish()
}

// ── Parser tests ──────────────────────────────────────────────────────────────

#[test]
fn parse_header_accepts_minimal_dtb() {
    let dtb = minimal_dtb();
    parse_header(&dtb).expect("should succeed");
}

#[test]
fn parse_header_rejects_bad_magic() {
    let mut dtb = minimal_dtb();
    dtb[0] = 0xFF;
    assert!(matches!(parse_header(&dtb), Err(ParseError::BadMagic)));
}

#[test]
fn parse_header_rejects_too_small() {
    assert!(matches!(parse_header(&[0u8; 4]), Err(ParseError::TooSmall)));
}

#[test]
fn fdt_iter_yields_begin_end_for_minimal_dtb() {
    let dtb = minimal_dtb();
    let hdr = parse_header(&dtb).unwrap();
    let mut iter = FdtIter::new(&dtb, &hdr);
    let ev0 = iter.next();
    let ev1 = iter.next();
    let ev2 = iter.next();
    assert!(matches!(ev0, Some(Ok(NodeEvent::BeginNode { .. }))));
    assert!(matches!(ev1, Some(Ok(NodeEvent::EndNode))));
    assert!(ev2.is_none());
}

// ── compat matching ───────────────────────────────────────────────────────────

#[test]
fn compat_matches_single_string() {
    assert!(matches_compat(b"virtio,mmio", ALLOWED_COMPAT_QEMU_VIRT));
}

#[test]
fn compat_matches_multi_string() {
    let multi = b"riscv-virtio\0virtio,mmio";
    assert!(matches_compat(multi, ALLOWED_COMPAT_QEMU_VIRT));
}

#[test]
fn compat_rejects_unknown_string() {
    assert!(!matches_compat(b"unknown,device", ALLOWED_COMPAT_QEMU_VIRT));
}

// ── derive_board_profile ──────────────────────────────────────────────────────

fn make_plic_dtb() -> [u8; 4096] {
    // Build a minimal DTB with a PLIC node so derive does not fail.
    let mut b = DtbBuilder::new();
    b.begin_node(b"");           // root node (depth 1)
      b.begin_node(b"plic@c000000");  // device node (depth 2)
        // compatible = "riscv,plic0\0"
        let compat = b"riscv,plic0\0";
        b.prop(0, compat);  // name_off=0 → "compatible"
        // reg = addr=0x0C000000 size=0x4000000 (big-endian 64-bit pairs)
        let mut reg = [0u8; 16];
        reg[0..8] .copy_from_slice(&0x0C00_0000u64.to_be_bytes());
        reg[8..16].copy_from_slice(&0x0400_0000u64.to_be_bytes());
        b.prop(11, &reg);  // name_off=11 → "reg"
      b.end_node();
    b.end_node();
    b.fdt_end();
    b.finish()
}

#[test]
fn derive_board_profile_succeeds_with_plic_dtb() {
    let dtb = make_plic_dtb();
    let platform = PlatformProfile::qemu_virt_default();
    let ctx = DeriveContext::qemu_virt_default(platform);
    let result = derive_board_profile(
        &dtb, &ctx,
        b"test-board\0\0\0\0\0\0",
        b"v05\0\0\0\0\0",
    );
    assert!(result.is_ok(), "expected Ok, got {:?}", result);
}

#[test]
fn derive_board_profile_has_plic_device() {
    let dtb = make_plic_dtb();
    let platform = PlatformProfile::qemu_virt_default();
    let ctx = DeriveContext::qemu_virt_default(platform);
    let bp = derive_board_profile(
        &dtb, &ctx, b"test\0\0\0\0\0\0\0\0\0\0\0\0", b"\0\0\0\0\0\0\0\0",
    ).unwrap();
    assert!(bp.device_count > 0);
    assert_eq!(bp.devices[0].class as u8, DeviceClass::Plic as u8);
}

#[test]
fn derive_board_profile_digest_is_nonzero() {
    let dtb = make_plic_dtb();
    let platform = PlatformProfile::qemu_virt_default();
    let ctx = DeriveContext::qemu_virt_default(platform);
    let bp = derive_board_profile(
        &dtb, &ctx, b"test\0\0\0\0\0\0\0\0\0\0\0\0", b"\0\0\0\0\0\0\0\0",
    ).unwrap();
    assert_ne!(bp.profile_digest.0, [0u8; 32]);
}

#[test]
fn derive_board_profile_fails_without_plic() {
    // Minimal DTB with no device nodes.
    let dtb = minimal_dtb();
    let platform = PlatformProfile::qemu_virt_default();
    let ctx = DeriveContext::qemu_virt_default(platform);
    let result = derive_board_profile(
        &dtb, &ctx, b"\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0", b"\0\0\0\0\0\0\0\0",
    );
    assert!(matches!(result, Err(DeriveError::MissingPlic)));
}
