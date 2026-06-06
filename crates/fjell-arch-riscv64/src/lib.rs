//! RISC-V 64-bit `ArchIdentity` implementation (RFC v0.5-003).
//!
//! All RISC-V-specific code lives here; `fjell-arch` and the kernel
//! import only the trait.
#![no_std]

use fjell_arch::{ArchIdentity};

/// RISC-V 64 GC architecture tag.
pub struct Riscv64Gc;

impl fjell_arch::sealed::Sealed for Riscv64Gc {}

impl ArchIdentity for Riscv64Gc {
    const ARCH_NAME: &'static str = "riscv64gc";
    const GP_REGS:   usize        = 32;   // x0..x31
    const PAGE_SIZE: usize        = 4096;
}

/// The active architecture for this build.
pub type ActiveArch = Riscv64Gc;

// ── RISC-V-specific constants ─────────────────────────────────────────────────

/// Supervisor page-table mode for Sv39 (written into satp.MODE).
pub const SATP_MODE_SV39: u64 = 8;
/// RISC-V PLIC base address on the QEMU `virt` machine.
pub const QEMU_PLIC_BASE: usize = 0x0C00_0000;
/// RISC-V CLINT base address on the QEMU `virt` machine.
pub const QEMU_CLINT_BASE: usize = 0x0200_0000;

// ── ISA extension detection ───────────────────────────────────────────────────

/// Read the `misa` CSR if available and return the extension bitmask.
///
/// On QEMU `virt`, `misa` is accessible in M-mode.  This function must
/// only be called during early kernel init (M-mode or S-mode with no MMU).
#[cfg(target_arch = "riscv64")]
pub fn read_misa() -> u64 {
    let val: u64;
    // SAFETY: invariants upheld by the surrounding context; see module documentation.
    unsafe {
        core::arch::asm!("csrr {}, misa", out(reg) val, options(nostack));
    }
    val
}

#[cfg(not(target_arch = "riscv64"))]
pub fn read_misa() -> u64 { 0 }
