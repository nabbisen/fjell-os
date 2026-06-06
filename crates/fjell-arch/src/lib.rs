//! Architecture-dependent primitives for Fjell OS.
//!
//! This crate isolates all CPU-specific code (CSR access, trap handling,
//! page-table primitives, interrupt controller, timer) so that the rest of
//! the kernel stays architecture-agnostic.
//!
//! Current target: `riscv64gc-unknown-none-elf` (RISC-V 64, QEMU `virt`).

#![no_std]

// Module stubs — filled in from M1/M2 onward.

/// RISC-V 64 specific implementations.
#[cfg(target_arch = "riscv64")]
pub mod riscv64 {
    /// Control and Status Register (CSR) access helpers.
    pub mod csr {}

    /// Supervisor Address Translation and Protection (`satp`) helpers.
    pub mod satp {}

    /// Page Table Entry definitions for Sv39.
    pub mod pte {}

    /// Trap entry and delegation configuration.
    pub mod trap {}

    /// Timer (CLINT) helpers.
    pub mod timer {}
}

// ── RFC v0.5-003: arch-neutral trait boundary ─────────────────────────────────

/// Virtual address.
pub type Va = usize;
/// Physical address.
pub type Pa = usize;
/// Address-Space Identifier.
pub type Asid = u16;

/// Page permission bits (arch-agnostic subset).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PagePerm(pub u8);
impl PagePerm {
    pub const READ:    Self = Self(1 << 0);
    pub const WRITE:   Self = Self(1 << 1);
    pub const EXECUTE: Self = Self(1 << 2);
    pub const USER:    Self = Self(1 << 3);
    pub const fn or(self, rhs: Self) -> Self { Self(self.0 | rhs.0) }
    pub fn has(self, p: Self) -> bool { (self.0 & p.0) == p.0 }
}

/// Opaque bag of general-purpose register values.
///
/// Only `fjell-arch-*` crates dereference the fields; outside code
/// must not access them directly (RFC v0.5-003 §6.1).
#[derive(Clone, Copy, Debug, Default)]
pub struct ArchRegs {
    /// 32 64-bit registers (x0..x31 for RISC-V; x0..x30+SP for ARM64).
    pub raw: [u64; 32],
}

/// Snapshot of architectural state at a trap entry.
#[derive(Clone, Copy, Debug)]
pub struct TrapFrame {
    /// Exception / interrupt return address.
    pub epc:    usize,
    /// Architecture status register snapshot (mstatus/SPSR).
    pub status: usize,
    /// Cause register (mcause / ESR_EL1).
    pub cause:  usize,
    /// Trap value register (mtval / FAR_EL1).
    pub tval:   usize,
    /// General-purpose register snapshot.
    pub regs:   ArchRegs,
}

/// Architecture identity sealed trait (RFC v0.5-003 §5.1).
///
/// Implementations live in `fjell-arch-riscv64` and `fjell-arch-arm64`.
/// External crates cannot implement this trait.
pub trait ArchIdentity: sealed::Sealed {
    /// Short lowercase string: `"riscv64gc"` or `"arm64"`.
    const ARCH_NAME: &'static str;
    /// Number of general-purpose registers.
    const GP_REGS: usize;
    /// Page size in bytes.
    const PAGE_SIZE: usize;
}

pub mod sealed {
    pub trait Sealed {}
}
