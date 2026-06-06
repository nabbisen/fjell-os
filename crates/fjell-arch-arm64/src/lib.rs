//! ARM64 architecture stub for Fjell OS — second-platform preparation
//! (RFC v0.5-003).
//!
//! This crate exists to prove the arch boundary compiles for a second
//! target.  Functional ARM64 bring-up is planned for v0.6+.
#![no_std]

use fjell_arch::ArchIdentity;

/// ARM64 (AArch64) architecture tag.
pub struct Arm64;

impl fjell_arch::sealed::Sealed for Arm64 {}

impl ArchIdentity for Arm64 {
    const ARCH_NAME: &'static str = "arm64";
    const GP_REGS:   usize        = 31;   // x0..x30 (SP is separate)
    const PAGE_SIZE: usize        = 4096;
}

/// The active architecture for ARM64 builds.
pub type ActiveArch = Arm64;

// ── ARM64-specific constants (stub values) ────────────────────────────────────

/// GIC distributor base address for a typical ARM64 QEMU `virt` machine.
pub const QEMU_GICD_BASE: usize = 0x0800_0000;
/// GIC CPU interface base address.
pub const QEMU_GICC_BASE: usize = 0x0801_0000;
