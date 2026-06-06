//! Host unit tests for `fjell-platform-format` (RFC v0.5-001 §11).

use crate::platform::{
    PlatformProfile, PlatformFamily, IsaExtensions, KernelAbiVersion,
    PLATFORM_PROFILE_VERSION, ISA_MANDATORY,
    ISA_EXT_I, ISA_EXT_M, ISA_EXT_A, ISA_EXT_F, ISA_EXT_D,
};
use crate::board::{
    BoardProfile, DeviceClass, RecoveryKind, BOARD_PROFILE_VERSION,
    MAX_BOARD_DEVICES,
};
use crate::isa::{reserved_bits_clear};
use crate::digest::{platform_digest, board_digest};
use fjell_measure_format::Digest32;

// ── PlatformProfile ───────────────────────────────────────────────────────────

#[test]
fn platform_profile_version_is_one() {
    assert_eq!(PLATFORM_PROFILE_VERSION, 1);
}

#[test]
fn qemu_virt_default_family_is_riscv64gc() {
    let p = PlatformProfile::qemu_virt_default();
    assert_eq!(p.family as u8, PlatformFamily::Riscv64Gc as u8);
}

#[test]
fn qemu_virt_default_isa_is_mandatory_compliant() {
    let p = PlatformProfile::qemu_virt_default();
    assert!(p.isa_extensions.is_riscv64gc_compliant(),
        "default ISA must include I, M, A extensions");
}

#[test]
fn isa_extensions_bitwise_ops() {
    let e = IsaExtensions::default().with(ISA_EXT_I).with(ISA_EXT_M);
    assert!(e.contains(ISA_EXT_I));
    assert!(e.contains(ISA_EXT_M));
    assert!(!e.contains(ISA_EXT_F));
}

#[test]
fn isa_reserved_bits_clear_for_known_set() {
    let e = IsaExtensions(ISA_EXT_I | ISA_EXT_M | ISA_EXT_A | ISA_EXT_F);
    assert!(reserved_bits_clear(e));
}

#[test]
fn isa_reserved_bits_set_detected() {
    let e = IsaExtensions(ISA_EXT_I | (1 << 63)); // reserved bit 63
    assert!(!reserved_bits_clear(e));
}

#[test]
fn kernel_abi_v0_5_values() {
    assert_eq!(KernelAbiVersion::V0_5.major, 0);
    assert_eq!(KernelAbiVersion::V0_5.minor, 5);
}

// ── BoardProfile ──────────────────────────────────────────────────────────────

#[test]
fn board_profile_version_is_one() {
    assert_eq!(BOARD_PROFILE_VERSION, 1);
}

#[test]
fn board_max_devices_is_sixteen() {
    assert_eq!(MAX_BOARD_DEVICES, 16);
}

#[test]
fn qemu_virt_board_has_expected_device_count() {
    let bp = BoardProfile::qemu_virt_default(Digest32([0u8; 32]));
    assert_eq!(bp.device_count, 5, "uart + net + blk + plic + clint");
}

#[test]
fn qemu_virt_board_first_device_is_uart() {
    let bp = BoardProfile::qemu_virt_default(Digest32([0u8; 32]));
    assert_eq!(bp.devices[0].class as u8, DeviceClass::Uart8250 as u8);
}

#[test]
fn board_recovery_kind_is_boot_arg() {
    let bp = BoardProfile::qemu_virt_default(Digest32([0u8; 32]));
    assert_eq!(bp.recovery.kind as u8, RecoveryKind::BootArg as u8);
}

#[test]
fn device_class_roundtrip() {
    for &(byte, expected) in &[
        (0x01u8, DeviceClass::Uart8250),
        (0x02,   DeviceClass::VirtioNetMmio),
        (0x03,   DeviceClass::VirtioBlkMmio),
        (0x05,   DeviceClass::Plic),
        (0xFF,   DeviceClass::Generic),
    ] {
        assert_eq!(DeviceClass::from_u8(byte).unwrap() as u8, expected as u8);
    }
}

#[test]
fn device_class_unknown_returns_none() {
    assert!(DeviceClass::from_u8(0xAB).is_none());
}

// ── Digest computation ────────────────────────────────────────────────────────

#[test]
fn platform_digest_is_nonzero() {
    let p = PlatformProfile::qemu_virt_default();
    let d = platform_digest(&p);
    assert_ne!(d.0, [0u8; 32]);
}

#[test]
fn platform_digest_is_deterministic() {
    let p = PlatformProfile::qemu_virt_default();
    let d1 = platform_digest(&p);
    let d2 = platform_digest(&p);
    assert_eq!(d1.0, d2.0);
}

#[test]
fn board_digest_is_nonzero() {
    let pd = platform_digest(&PlatformProfile::qemu_virt_default());
    let bp = BoardProfile::qemu_virt_default(pd);
    let bd = board_digest(&bp);
    assert_ne!(bd.0, [0u8; 32]);
}

#[test]
fn board_digest_binds_platform_ref() {
    let pd1 = platform_digest(&PlatformProfile::qemu_virt_default());
    let pd2 = Digest32([0x42u8; 32]);  // different platform
    let bd1 = board_digest(&BoardProfile::qemu_virt_default(pd1));
    let bd2 = board_digest(&BoardProfile::qemu_virt_default(pd2));
    assert_ne!(bd1.0, bd2.0,
        "board digest must differ when platform_ref differs");
}

#[test]
fn digest_chain_parity() {
    // Verify the full profile–board chain: platform computed, used as
    // platform_ref, board computed.  The final board_digest encodes the
    // full dependency tree.
    let mut pp = PlatformProfile::qemu_virt_default();
    pp.profile_digest = platform_digest(&pp);
    let mut bp = BoardProfile::qemu_virt_default(pp.profile_digest);
    bp.profile_digest = board_digest(&bp);
    assert_ne!(bp.profile_digest.0, [0u8; 32]);
    // Re-verify is idempotent.
    assert_eq!(bp.profile_digest.0, board_digest(&bp).0);
}
