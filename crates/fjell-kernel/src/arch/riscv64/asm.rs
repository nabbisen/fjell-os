//! Low-level assembly helpers that do not belong to a specific CSR module.

/// Execute the `wfi` (Wait For Interrupt) instruction.
///
/// Used by the idle task to reduce power consumption while no runnable task
/// is available.
///
/// # Safety
/// Must be called with interrupts enabled; otherwise the processor will spin
/// indefinitely.
#[inline]
pub unsafe fn wfi() {
    // SAFETY: caller ensures interrupts are enabled so the hart can wake up.
    unsafe { core::arch::asm!("wfi") };
}
