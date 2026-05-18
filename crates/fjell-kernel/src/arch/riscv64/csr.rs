//! RISC-V Control and Status Register (CSR) access helpers.
//!
//! All functions are `unsafe`. Many are unused in M2 but are required by
//! M3+ (IPC, capability, SMP) and are intentionally kept for future milestones.

#![allow(dead_code)]

/// Read `mhartid`.
///
/// # Safety
/// Must be called from M-mode.
#[inline]
pub unsafe fn read_mhartid() -> usize {
    let v: usize;
    // SAFETY: read-only CSR, no side effects.
    unsafe { core::arch::asm!("csrr {}, mhartid", out(reg) v) };
    v
}

/// Read `sstatus`.
#[inline]
pub unsafe fn read_sstatus() -> usize {
    let v: usize;
    // SAFETY: read-only CSR access.
    unsafe { core::arch::asm!("csrr {}, sstatus", out(reg) v) };
    v
}

/// Write `sstatus`.
///
/// # Safety
/// Caller must ensure the new value preserves required kernel invariants
/// (e.g. SIE, SPP bits).
#[inline]
pub unsafe fn write_sstatus(v: usize) {
    // SAFETY: caller upholds CSR write invariants.
    unsafe { core::arch::asm!("csrw sstatus, {}", in(reg) v) };
}

/// Read `scause`.
#[inline]
pub unsafe fn read_scause() -> usize {
    let v: usize;
    // SAFETY: read-only CSR access.
    unsafe { core::arch::asm!("csrr {}, scause", out(reg) v) };
    v
}

/// Read `stval`.
#[inline]
pub unsafe fn read_stval() -> usize {
    let v: usize;
    // SAFETY: read-only CSR access.
    unsafe { core::arch::asm!("csrr {}, stval", out(reg) v) };
    v
}

/// Read `sepc`.
#[inline]
pub unsafe fn read_sepc() -> usize {
    let v: usize;
    // SAFETY: read-only CSR access.
    unsafe { core::arch::asm!("csrr {}, sepc", out(reg) v) };
    v
}

/// Write `sepc`.
///
/// # Safety
/// `v` must be a valid canonical user-space virtual address.
#[inline]
pub unsafe fn write_sepc(v: usize) {
    // SAFETY: caller guarantees `v` is a valid user-space address.
    unsafe { core::arch::asm!("csrw sepc, {}", in(reg) v) };
}

/// Write `stvec` (trap-vector base address, direct mode).
///
/// # Safety
/// `addr` must be 4-byte aligned and point to the trap entry assembly stub.
#[inline]
pub unsafe fn write_stvec(addr: usize) {
    // SAFETY: caller ensures `addr` points to a valid trap handler with
    // correct alignment and that MODE = 0 (direct).
    unsafe { core::arch::asm!("csrw stvec, {}", in(reg) addr) };
}

/// Read `sscratch`.
#[inline]
pub unsafe fn read_sscratch() -> usize {
    let v: usize;
    // SAFETY: read-only CSR access.
    unsafe { core::arch::asm!("csrr {}, sscratch", out(reg) v) };
    v
}

/// Write `sscratch`.
///
/// # Safety
/// Caller manages the value stored in `sscratch` (used as per-hart pointer).
#[inline]
pub unsafe fn write_sscratch(v: usize) {
    // SAFETY: caller owns the sscratch convention.
    unsafe { core::arch::asm!("csrw sscratch, {}", in(reg) v) };
}

/// Write `sie` (supervisor interrupt-enable).
///
/// # Safety
/// Enabling interrupts at the wrong time can cause re-entrant trap handling.
#[inline]
pub unsafe fn write_sie(v: usize) {
    // SAFETY: caller is responsible for interrupt-enable timing.
    unsafe { core::arch::asm!("csrw sie, {}", in(reg) v) };
}

/// Enable supervisor external and timer interrupts.
///
/// # Safety
/// Must only be called after `stvec` is installed and the trap handler is ready.
#[inline]
pub unsafe fn enable_interrupts() {
    // SEIE (bit 9) | STIE (bit 5) | SSIE (bit 1)
    unsafe { write_sie(0x222) };
    // Set SIE bit in sstatus to globally enable S-mode interrupts.
    let s = unsafe { read_sstatus() };
    unsafe { write_sstatus(s | (1 << 1)) }; // SIE = bit 1
}

// ── M-mode CSRs (only accessible from M-mode shim) ──────────────────────────

/// Write `mstatus`.
///
/// # Safety
/// Must be called from M-mode only.
#[inline]
pub unsafe fn write_mstatus(v: usize) {
    // SAFETY: M-mode shim context; caller upholds M-mode invariants.
    unsafe { core::arch::asm!("csrw mstatus, {}", in(reg) v) };
}

/// Write `medeleg` (machine exception delegation).
///
/// # Safety
/// Delegating the wrong exceptions to S-mode can break kernel trap handling.
#[inline]
pub unsafe fn write_medeleg(v: usize) {
    // SAFETY: caller selects only safe-to-delegate exception bits.
    unsafe { core::arch::asm!("csrw medeleg, {}", in(reg) v) };
}

/// Write `mideleg` (machine interrupt delegation).
///
/// # Safety
/// Delegating the wrong interrupts can break timer and IPI handling.
#[inline]
pub unsafe fn write_mideleg(v: usize) {
    // SAFETY: caller selects only safe-to-delegate interrupt bits.
    unsafe { core::arch::asm!("csrw mideleg, {}", in(reg) v) };
}

/// Write `mepc` (machine exception program counter).
///
/// # Safety
/// `v` must point to the S-mode entry function.
#[inline]
pub unsafe fn write_mepc(v: usize) {
    // SAFETY: caller provides the valid S-mode entry address.
    unsafe { core::arch::asm!("csrw mepc, {}", in(reg) v) };
}

/// Execute `mret` to return from M-mode to the privilege level encoded in `mstatus.MPP`.
///
/// # Safety
/// `mepc` and `mstatus.MPP` must be correctly set up before calling this.
/// This function does not return; control transfers to `mepc`.
#[inline]
pub unsafe fn mret() -> ! {
    // SAFETY: caller has correctly set mepc (S-mode entry) and mstatus.MPP = S.
    unsafe {
        core::arch::asm!("mret", options(noreturn));
    }
}

/// Write `pmpaddr0` (Physical Memory Protection address register 0).
///
/// # Safety
/// Must be called from M-mode.  Incorrect values can lock out all memory access.
#[inline]
pub unsafe fn write_pmpaddr0(v: usize) {
    unsafe { core::arch::asm!("csrw pmpaddr0, {}", in(reg) v) };
}

/// Write `pmpcfg0` (Physical Memory Protection configuration register 0).
///
/// # Safety
/// Must be called from M-mode.  Incorrect values can lock out all memory access.
/// Once a PMP entry is locked (L-bit set) it cannot be changed until reset.
#[inline]
pub unsafe fn write_pmpcfg0(v: usize) {
    unsafe { core::arch::asm!("csrw pmpcfg0, {}", in(reg) v) };
}
