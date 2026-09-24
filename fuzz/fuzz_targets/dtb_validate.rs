#![no_main]
//! Fuzzes both readers of a firmware-supplied device tree:
//!
//! * `fdt_extent` (`fjell-fdt-header`), the 8-byte header check the kernel runs on the
//!   pointer firmware hands it, **on every boot** (RFC-0.33-001 D22);
//! * `validate_dtb` (`fjell-dtb-validate`), the full boot-handoff validator, which has
//!   no caller yet (E-048) but will meet firmware bytes at hardware bring-up (E-004).
//!
//! The seed is the one committed tree, `corpora/dtb_validate/qemu-virt-bios-none.dtb`
//! (5,044 bytes, a dump of QEMU `virt`'s) -- the same file `fjell-dtb-validate`'s own
//! test validates, not a copy (RFC-0.33-005 §B). The target `dtb_derive_board_profile`
//! that shared it was deleted with `fjell-dtb-derive`.
use fjell_dtb_validate::validate_dtb;
use fjell_fdt_header::fdt_extent;
use fjell_platform_format::{platform_digest, BoardProfile, PlatformProfile};
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    // The kernel's header check must never panic, and must never claim more than it
    // was given room for beyond its own ceiling.
    if let Ok(n) = fdt_extent(data) {
        assert!(n <= fjell_fdt_header::FDT_MAX_TOTALSIZE as usize);
    }
    let pp = PlatformProfile::qemu_virt_default();
    let bp = BoardProfile::qemu_virt_default(platform_digest(&pp));
    let _ = validate_dtb(data, &bp);
});
