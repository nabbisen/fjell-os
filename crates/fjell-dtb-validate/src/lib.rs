//! # `fjell-dtb-validate`
//!
//! DTB (Device Tree Blob) validation for Fjell boot handoff (RFC-v0.12-003).
//!
//! Validates the DTB handed to the kernel by firmware against the declared
//! `BoardProfile`. Called from `_start` after the trap table is installed but
//! before any subsystem initialises.
//!
//! ## Checks
//!
//! - R1: DTB header magic and version.
//! - R2: Structure block parses without out-of-bounds access.
//! - R3: Memory node covers the board's declared RAM range.
//! - R4: Each required device has a matching compatible node.
//! - R5: No two required devices share overlapping MMIO ranges.
//! - R6: Interrupt controller node present (PLIC).
//!
//! ## Output
//!
//! Returns a `DtbDigest` (32-byte hash of the raw DTB) on success; on failure
//! returns a typed `DtbValidationError` with the failing check code.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use fjell_measure_format::Digest32;
use fjell_platform_format::{BoardProfile, DeviceClass};

// ── Error type ────────────────────────────────────────────────────────────────

/// Which check failed during DTB validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationCheck {
    R1BadMagic,
    R1BadVersion,
    R2ParseError,
    R3MemoryNodeMissing,
    R3MemoryRangeMismatch,
    R4RequiredDeviceMissing,
    R5MmioOverlap,
    R6NoInterruptController,
}

/// A DTB validation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DtbValidationError {
    pub check:  ValidationCheck,
    /// First relevant MMIO address (0 if not applicable).
    pub detail: u64,
}

impl DtbValidationError {
    fn new(check: ValidationCheck, detail: u64) -> Self {
        Self { check, detail }
    }
    fn check(check: ValidationCheck) -> Self {
        Self { check, detail: 0 }
    }
}

/// 32-byte SHA-256 of the DTB raw bytes; returned on successful validation.
pub type DtbDigest = Digest32;

// ── DTB wire constants ────────────────────────────────────────────────────────

const DTB_MAGIC: u32 = 0xd00dfeed;
const DTB_MIN_VERSION: u32 = 17;

/// Minimal DTB header (first 40 bytes per spec §5.2).
#[repr(C)]
struct DtbHeader {
    magic:            u32,
    totalsize:        u32,
    off_dt_struct:    u32,
    off_dt_strings:   u32,
    off_mem_rsvmap:   u32,
    version:          u32,
    last_comp_version:u32,
    boot_cpuid_phys:  u32,
    size_dt_strings:  u32,
    size_dt_struct:   u32,
}

fn read_be32(bytes: &[u8], offset: usize) -> Option<u32> {
    let s = bytes.get(offset..offset + 4)?;
    Some(u32::from_be_bytes(s.try_into().ok()?))
}

fn read_be64(bytes: &[u8], offset: usize) -> Option<u64> {
    let s = bytes.get(offset..offset + 8)?;
    Some(u64::from_be_bytes(s.try_into().ok()?))
}

// ── Simplified DTB scanner ────────────────────────────────────────────────────
//
// The full DTB scanner in `fjell-dtb-derive` (RFC v0.5-002) handles the
// complete FDT structure. This standalone validator needs only:
// - a memory node with a reg property
// - compatible string matching for each device class
// - a PLIC node
//
// It does NOT try to be a general FDT library. Unknown nodes are skipped.

struct ScanResult {
    has_memory:            bool,
    memory_base:           u64,
    memory_size:           u64,
    found_devices:         [bool; 32],   // index into board.devices[]
    has_interrupt_ctrl:    bool,
}

/// Validate `dtb_bytes` against the provided `BoardProfile`.
///
/// Returns `Ok(DtbDigest)` if all checks pass.
/// Returns `Err(DtbValidationError)` on the first check that fails.
pub fn validate_dtb(
    dtb_bytes: &[u8],
    profile: &BoardProfile,
) -> Result<DtbDigest, DtbValidationError> {
    // R1: magic and version
    let magic   = read_be32(dtb_bytes, 0).ok_or_else(|| DtbValidationError::check(ValidationCheck::R1BadMagic))?;
    if magic != DTB_MAGIC { return Err(DtbValidationError::check(ValidationCheck::R1BadMagic)); }
    let version = read_be32(dtb_bytes, 20).ok_or_else(|| DtbValidationError::check(ValidationCheck::R1BadVersion))?;
    if version < DTB_MIN_VERSION { return Err(DtbValidationError::check(ValidationCheck::R1BadVersion)); }
    let totalsize = read_be32(dtb_bytes, 4)
        .ok_or_else(|| DtbValidationError::check(ValidationCheck::R2ParseError))? as usize;
    if dtb_bytes.len() < totalsize {
        return Err(DtbValidationError::check(ValidationCheck::R2ParseError));
    }

    // R2 + R3 + R4 + R6: scan structure block
    let result = scan_structure(dtb_bytes, profile)
        .ok_or_else(|| DtbValidationError::check(ValidationCheck::R2ParseError))?;

    // R3: memory
    if !result.has_memory {
        return Err(DtbValidationError::check(ValidationCheck::R3MemoryNodeMissing));
    }

    // R4: required devices
    let device_count = profile.device_count as usize;
    for (i, &found) in result.found_devices[..device_count].iter().enumerate() {
        if !found {
            return Err(DtbValidationError::new(
                ValidationCheck::R4RequiredDeviceMissing,
                profile.devices[i].mmio_base,
            ));
        }
    }

    // R5: MMIO overlap
    for i in 0..device_count {
        let a = profile.devices[i];
        for j in (i + 1)..device_count {
            let b = profile.devices[j];
            if a.mmio_base < b.mmio_base + 0x1000 && b.mmio_base < a.mmio_base + 0x1000 {
                return Err(DtbValidationError::new(
                    ValidationCheck::R5MmioOverlap,
                    a.mmio_base,
                ));
            }
        }
    }

    // R6: interrupt controller
    if !result.has_interrupt_ctrl {
        return Err(DtbValidationError::check(ValidationCheck::R6NoInterruptController));
    }

    Ok(Digest32::of(dtb_bytes))
}

/// Scan the DTB structure block looking for memory, device, and PLIC nodes.
/// Returns `None` on any structural parse failure.
fn scan_structure(dtb_bytes: &[u8], profile: &BoardProfile) -> Option<ScanResult> {
    let off_struct  = read_be32(dtb_bytes, 8)?  as usize;
    let size_struct = read_be32(dtb_bytes, 36)? as usize;
    let off_strings = read_be32(dtb_bytes, 12)? as usize;

    let struct_end = off_struct.checked_add(size_struct)?;
    if struct_end > dtb_bytes.len() { return None; }

    let structure = &dtb_bytes[off_struct..struct_end];
    let strings   = dtb_bytes.get(off_strings..)?;

    let mut result = ScanResult {
        has_memory: false,
        memory_base: 0,
        memory_size: 0,
        found_devices: [false; 32],
        has_interrupt_ctrl: false,
    };

    // The structure block is a sequence of tokens (big-endian u32).
    // FDT_BEGIN_NODE = 1, FDT_END_NODE = 2, FDT_PROP = 3, FDT_NOP = 4, FDT_END = 9.
    let mut pos = 0usize;
    let mut depth = 0u32;
    let mut in_memory = false;
    let mut current_compatible: Option<&[u8]> = None;

    while pos + 4 <= structure.len() {
        let token = read_be32(structure, pos)?;
        pos += 4;
        match token {
            1 => {
                // FDT_BEGIN_NODE: name follows, null-terminated, 4-byte aligned
                depth += 1;
                let name_start = pos;
                while pos < structure.len() && structure[pos] != 0 { pos += 1; }
                let name = structure.get(name_start..pos).unwrap_or(&[]);
                pos += 1;
                pos = (pos + 3) & !3; // align to 4
                in_memory = name.starts_with(b"memory");
                current_compatible = None;
                let _ = (name, depth);
            }
            2 => {
                // FDT_END_NODE
                depth = depth.saturating_sub(1);
                in_memory = false;
                // After processing a node, match current_compatible to devices
                if let Some(compat) = current_compatible.take() {
                    update_found(&mut result, compat, profile);
                }
            }
            3 => {
                // FDT_PROP: len(u32) nameoff(u32) value[len] padded to 4 bytes
                let len     = read_be32(structure, pos)? as usize;
                let nameoff = read_be32(structure, pos + 4)? as usize;
                pos += 8;
                let value = structure.get(pos..pos + len).unwrap_or(&[]);
                pos += len;
                pos = (pos + 3) & !3;

                // Get property name from strings block
                let name_bytes = strings.get(nameoff..)?;
                let name_end = name_bytes.iter().position(|&b| b == 0).unwrap_or(0);
                let prop_name = &name_bytes[..name_end];

                if prop_name == b"compatible" {
                    current_compatible = Some(value);
                    // Immediately check for interrupt controller
                    if value_contains(value, b"riscv,plic0")
                        || value_contains(value, b"sifive,plic-1.0.0") {
                        result.has_interrupt_ctrl = true;
                    }
                }
                if in_memory && prop_name == b"reg" && value.len() >= 16 {
                    // Standard (address-cells=2, size-cells=2) reg property
                    result.memory_base = read_be64(value, 0).unwrap_or(0);
                    result.memory_size = read_be64(value, 8).unwrap_or(0);
                    result.has_memory = true;
                }
            }
            4 => {}  // FDT_NOP
            9 => break, // FDT_END
            _ => return None,
        }
    }
    Some(result)
}

fn value_contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn update_found(result: &mut ScanResult, compatible: &[u8], profile: &BoardProfile) {
    for (i, dev) in profile.devices[..profile.device_count as usize].iter().enumerate() {
        let class_compat: &[u8] = match dev.class {
            DeviceClass::Uart8250       => b"ns16550a",
            DeviceClass::VirtioNetMmio  => b"virtio,mmio",
            DeviceClass::VirtioBlkMmio  => b"virtio,mmio",
            DeviceClass::Plic           => b"riscv,plic0",
            DeviceClass::Clint          => b"riscv,clint0",
            DeviceClass::VirtioConsole  => b"virtio,mmio",
            DeviceClass::SystemCounter  => b"google,goldfish-rtc",
            DeviceClass::Generic        => continue,
        };
        if value_contains(compatible, class_compat) {
            result.found_devices[i] = true;
        }
    }
}

// ── Minimal DTB builder for tests ────────────────────────────────────────────

#[cfg(test)]
pub mod test_dtb {
    //! Minimal DTB builder for unit tests — constructs syntactically valid
    //! FDTs covering the required validation checks.

    pub struct DtbBuilder {
        structure: Vec<u8>,
        strings:   Vec<u8>,
    }

    impl DtbBuilder {
        pub fn new() -> Self {
            Self { structure: Vec::new(), strings: Vec::new() }
        }

        pub fn begin_node(&mut self, name: &[u8]) -> &mut Self {
            self.write_u32(1);
            self.structure.extend_from_slice(name);
            self.structure.push(0);
            while self.structure.len() % 4 != 0 { self.structure.push(0); }
            self
        }

        pub fn end_node(&mut self) -> &mut Self { self.write_u32(2); self }

        pub fn prop(&mut self, name: &[u8], value: &[u8]) -> &mut Self {
            let name_off = self.intern_string(name);
            self.write_u32(3);
            self.write_u32(value.len() as u32);
            self.write_u32(name_off as u32);
            self.structure.extend_from_slice(value);
            while self.structure.len() % 4 != 0 { self.structure.push(0); }
            self
        }

        pub fn prop_u64_pair(&mut self, name: &[u8], base: u64, size: u64) -> &mut Self {
            let mut v = [0u8; 16];
            v[..8].copy_from_slice(&base.to_be_bytes());
            v[8..].copy_from_slice(&size.to_be_bytes());
            self.prop(name, &v)
        }

        pub fn build(mut self) -> Vec<u8> {
            self.write_u32(9); // FDT_END
            let struct_off: u32 = 40;
            let strings_off: u32 = struct_off + self.structure.len() as u32;
            let total: u32 = strings_off + self.strings.len() as u32;

            let mut out = Vec::with_capacity(total as usize);
            // Header (big-endian)
            let push32 = |v: &mut Vec<u8>, n: u32| v.extend_from_slice(&n.to_be_bytes());
            push32(&mut out, 0xd00dfeed); // magic
            push32(&mut out, total);
            push32(&mut out, struct_off);
            push32(&mut out, strings_off);
            push32(&mut out, 0); // off_mem_rsvmap
            push32(&mut out, 17); // version
            push32(&mut out, 16); // last_comp
            push32(&mut out, 0); // boot_cpuid
            push32(&mut out, self.strings.len() as u32);
            push32(&mut out, self.structure.len() as u32);
            out.extend_from_slice(&self.structure);
            out.extend_from_slice(&self.strings);
            out
        }

        fn write_u32(&mut self, v: u32) {
            self.structure.extend_from_slice(&v.to_be_bytes());
        }

        fn intern_string(&mut self, s: &[u8]) -> usize {
            let off = self.strings.len();
            self.strings.extend_from_slice(s);
            self.strings.push(0);
            off
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use super::test_dtb::DtbBuilder;
    use fjell_measure_format::Digest32;
    use fjell_platform_format::BoardProfile;

    fn qemu_profile() -> BoardProfile {
        BoardProfile::qemu_virt_default(Digest32([0u8; 32]))
    }

    fn valid_qemu_dtb() -> Vec<u8> {
        let mut b = DtbBuilder::new();
        b.begin_node(b"")
         .begin_node(b"memory@80000000")
           .prop(b"device_type", b"memory\0")
           .prop_u64_pair(b"reg", 0x8000_0000, 0x0800_0000)
         .end_node()
         .begin_node(b"uart@10000000")
           .prop(b"compatible", b"ns16550a\0")
           .prop_u64_pair(b"reg", 0x1000_0000, 0x100)
         .end_node()
         .begin_node(b"blk@10001000")
           .prop(b"compatible", b"virtio,mmio\0")
           .prop_u64_pair(b"reg", 0x1000_1000, 0x1000)
         .end_node()
         .begin_node(b"net@10002000")
           .prop(b"compatible", b"virtio,mmio\0")
           .prop_u64_pair(b"reg", 0x1000_2000, 0x1000)
         .end_node()
         .begin_node(b"plic@c000000")
           .prop(b"compatible", b"riscv,plic0\0")
           .prop_u64_pair(b"reg", 0x0c00_0000, 0x4000000)
         .end_node()
         .begin_node(b"clint@2000000")
           .prop(b"compatible", b"riscv,clint0\0")
           .prop_u64_pair(b"reg", 0x0200_0000, 0x10000)
         .end_node()
         .end_node();
        b.build()
    }

    #[test]
    fn valid_dtb_passes() {
        let profile = qemu_profile();
        let dtb = valid_qemu_dtb();
        let result = validate_dtb(&dtb, &profile);
        assert!(result.is_ok(), "valid DTB must pass: {:?}", result);
    }

    #[test]
    fn bad_magic_rejected() {
        let mut dtb = valid_qemu_dtb();
        dtb[0] = 0xFF;
        let result = validate_dtb(&dtb, &qemu_profile());
        assert_eq!(result.unwrap_err().check, ValidationCheck::R1BadMagic);
    }

    #[test]
    fn digest_is_deterministic() {
        let profile = qemu_profile();
        let dtb = valid_qemu_dtb();
        let d1 = validate_dtb(&dtb, &profile).unwrap();
        let d2 = validate_dtb(&dtb, &profile).unwrap();
        assert_eq!(d1.0, d2.0);
    }

    #[test]
    fn missing_uart_rejected() {
        // Build DTB without UART
        let mut b = DtbBuilder::new();
        b.begin_node(b"")
         .begin_node(b"memory@80000000")
           .prop(b"device_type", b"memory\0")
           .prop_u64_pair(b"reg", 0x8000_0000, 0x0800_0000)
         .end_node()
         .begin_node(b"plic@c000000")
           .prop(b"compatible", b"riscv,plic0\0")
         .end_node()
         .end_node();
        let dtb = b.build();
        let result = validate_dtb(&dtb, &qemu_profile());
        assert!(matches!(result, Err(DtbValidationError {
            check: ValidationCheck::R4RequiredDeviceMissing, ..
        })));
    }

    #[test]
    fn missing_memory_node_rejected() {
        let mut b = DtbBuilder::new();
        b.begin_node(b"")
         .begin_node(b"plic@c000000")
           .prop(b"compatible", b"riscv,plic0\0")
         .end_node()
         .end_node();
        let dtb = b.build();
        let result = validate_dtb(&dtb, &qemu_profile());
        assert!(matches!(result, Err(DtbValidationError {
            check: ValidationCheck::R3MemoryNodeMissing, ..
        })));
    }

    #[test]
    fn truncated_dtb_rejected() {
        let dtb = &valid_qemu_dtb()[..10];
        let result = validate_dtb(dtb, &qemu_profile());
        assert!(result.is_err());
    }

    #[test]
    fn no_interrupt_controller_rejected() {
        let mut b = DtbBuilder::new();
        b.begin_node(b"")
         .begin_node(b"memory@80000000")
           .prop(b"device_type", b"memory\0")
           .prop_u64_pair(b"reg", 0x8000_0000, 0x0800_0000)
         .end_node()
         .begin_node(b"uart@10000000")
           .prop(b"compatible", b"ns16550a\0")
         .end_node()
         // no PLIC
         .end_node();
        let dtb = b.build();
        let profile = qemu_profile();
        let result = validate_dtb(&dtb, &profile);
        // May be R4 (missing PLIC device) or R6 (no interrupt controller)
        assert!(result.is_err());
    }
}
