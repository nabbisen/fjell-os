//! # `fjell-fdt-header`
//!
//! The one thing the kernel checks about the device tree firmware hands it: that the
//! bytes at the pointer *are* a flattened device tree, and how long it is
//! (RFC-0.33-001 D22, erratum E-064). Eight bytes read, on **every** boot, before
//! anything is stored or reserved.
//!
//! **Why this is its own crate (RFC-0.33-005 D6).** It used to live in
//! `fjell-dtb-validate`, which carries a 600-line boot-handoff validator and two
//! format crates (`fjell-platform-format`, `fjell-measure-format`) that this code
//! uses neither of. The kernel is the most privileged component in the tree; it
//! should not link a validator, and two crates, and `fjell-canon` behind them, to
//! read eight bytes. So the kernel depends on this, which depends on nothing, and
//! `fjell-dtb-validate` depends on it for the magic.
//!
//! **What is not here, on purpose.** No token walk, no node parser, no board
//! profile. Full validation of a tree against a `BoardProfile` is hardware
//! bring-up (E-004) and lives in `fjell-dtb-validate`, called by nothing yet.
//!
//! **The committed tree.** `fuzz/corpora/dtb_validate/qemu-virt-bios-none.dtb` is a
//! dump of QEMU's `virt` tree, 5,044 bytes — the extent E-064 reserves at boot. The
//! `fjell-dtb-validate` test that validates it and the fuzz target that seeds from
//! it read that one file.

#![no_std]
#![forbid(unsafe_code)]

/// The flattened-device-tree magic, big-endian on the wire.
pub const FDT_MAGIC: u32 = 0xD00D_FEED;

/// Bytes of the fixed FDT header: enough to read the magic and `totalsize`.
pub const FDT_HEADER_PROBE_BYTES: usize = 8;

/// The smallest a real FDT can be: its own header (40 bytes, version 17).
const FDT_MIN_TOTALSIZE: u32 = 40;

/// A generous ceiling on `totalsize`. QEMU's `virt` tree is a few KiB; a value
/// beyond this is a wrong pointer, not a bigger tree.
pub const FDT_MAX_TOTALSIZE: u32 = 1 << 20;

/// Why the bytes at a claimed DTB pointer are not a device tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FdtHeaderError {
    /// Fewer than [`FDT_HEADER_PROBE_BYTES`] bytes were supplied.
    Short,
    /// The first four bytes are not the FDT magic. The value read is kept so a
    /// boot log can say what was there instead.
    BadMagic { found: u32 },
    /// `totalsize` is smaller than the header or larger than any tree we accept.
    BadTotalSize { found: u32 },
}

/// The extent, in bytes, of the device tree whose first bytes are `header` —
/// after checking that they *are* a device tree.
///
/// This is the check the kernel makes **before storing or reserving anything**
/// from the pointer firmware passes. Until E-064 was found the pointer was
/// `__bss_end`, a kernel address, and nothing looked at what it pointed to.
pub fn fdt_extent(header: &[u8]) -> Result<usize, FdtHeaderError> {
    let Some(head) = header.get(..FDT_HEADER_PROBE_BYTES) else {
        return Err(FdtHeaderError::Short);
    };
    let magic = u32::from_be_bytes([head[0], head[1], head[2], head[3]]);
    if magic != FDT_MAGIC {
        return Err(FdtHeaderError::BadMagic { found: magic });
    }
    let total = u32::from_be_bytes([head[4], head[5], head[6], head[7]]);
    if !(FDT_MIN_TOTALSIZE..=FDT_MAX_TOTALSIZE).contains(&total) {
        return Err(FdtHeaderError::BadTotalSize { found: total });
    }
    Ok(total as usize)
}

#[cfg(test)]
mod fdt_extent_tests {
    use super::*;

    fn header(magic: u32, total: u32) -> [u8; 8] {
        let mut h = [0u8; 8];
        h[..4].copy_from_slice(&magic.to_be_bytes());
        h[4..].copy_from_slice(&total.to_be_bytes());
        h
    }

    #[test]
    fn a_real_header_gives_its_total_size() {
        assert_eq!(fdt_extent(&header(FDT_MAGIC, 4096)), Ok(4096));
        assert_eq!(fdt_extent(&header(FDT_MAGIC, 40)), Ok(40));
        assert_eq!(
            fdt_extent(&header(FDT_MAGIC, FDT_MAX_TOTALSIZE)),
            Ok(FDT_MAX_TOTALSIZE as usize)
        );
    }

    #[test]
    fn the_byte_order_is_big_endian() {
        // The magic as it is laid out in memory: d0 0d fe ed.
        let mut h = [0u8; 8];
        h[..4].copy_from_slice(&[0xD0, 0x0D, 0xFE, 0xED]);
        h[4..].copy_from_slice(&[0, 0, 0x10, 0]);
        assert_eq!(fdt_extent(&h), Ok(0x1000));
        // Little-endian magic is not a device tree.
        h[..4].copy_from_slice(&[0xED, 0xFE, 0x0D, 0xD0]);
        assert!(matches!(
            fdt_extent(&h),
            Err(FdtHeaderError::BadMagic { .. })
        ));
    }

    #[test]
    fn the_value_the_kernel_used_to_receive_is_refused() {
        // E-064: dtb_pa was `__bss_end`, so what it pointed at was BSS — zeros.
        assert_eq!(
            fdt_extent(&[0u8; 8]),
            Err(FdtHeaderError::BadMagic { found: 0 })
        );
    }

    #[test]
    fn a_wrong_total_size_is_refused() {
        assert_eq!(
            fdt_extent(&header(FDT_MAGIC, 39)),
            Err(FdtHeaderError::BadTotalSize { found: 39 })
        );
        assert_eq!(
            fdt_extent(&header(FDT_MAGIC, FDT_MAX_TOTALSIZE + 1)),
            Err(FdtHeaderError::BadTotalSize {
                found: FDT_MAX_TOTALSIZE + 1
            })
        );
        assert_eq!(
            fdt_extent(&header(FDT_MAGIC, 0)),
            Err(FdtHeaderError::BadTotalSize { found: 0 })
        );
    }

    #[test]
    fn a_short_slice_is_refused_not_read_past() {
        assert_eq!(fdt_extent(&[]), Err(FdtHeaderError::Short));
        assert_eq!(
            fdt_extent(&[0xD0, 0x0D, 0xFE, 0xED, 0, 0, 0]),
            Err(FdtHeaderError::Short)
        );
    }
}
