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
