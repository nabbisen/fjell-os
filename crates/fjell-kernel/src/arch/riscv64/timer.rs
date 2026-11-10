//! CLINT (Core Local Interruptor) timer helpers for QEMU `virt`.
//!
//! The CLINT on QEMU virt is memory-mapped at `0x0200_0000`.
//! `mtime` is a 64-bit free-running counter at offset `0xBFF8`.
//! `mtimecmp[hart]` is at offset `0x4000 + hart * 8`.

/// CLINT base address on QEMU virt.
pub const CLINT_BASE: usize = 0x0200_0000;
/// Offset of `mtime` register.
pub const CLINT_MTIME: usize = CLINT_BASE + 0xBFF8;
/// Offset of `mtimecmp` for hart 0.
pub const CLINT_MTIMECMP0: usize = CLINT_BASE + 0x4000;

/// Approximate timer ticks per millisecond (QEMU virt clock = 10 MHz).
pub const TICKS_PER_MS: u64 = 10_000;
/// Scheduler preemption interval: 10 ms.
pub const TICK_INTERVAL: u64 = 10 * TICKS_PER_MS;

/// Frequency of the counter, in Hz: QEMU `virt`'s `timebase-frequency` (10 MHz).
///
/// The one platform fact the clock syscall's unit rests on. `sys_clock_now_ns` returns
/// nanoseconds so no caller needs it (RFC-0.42-002 R2); the kernel does, here.
pub const TIMEBASE_HZ: u64 = 10_000_000;

/// Nanoseconds per counter tick: exact only if the frequency divides 10^9, which the
/// assertion below makes a build error rather than a silent rounding.
pub const NS_PER_TICK: u64 = 1_000_000_000 / TIMEBASE_HZ;
const _: () = assert!(1_000_000_000 % TIMEBASE_HZ == 0);
// And it agrees with `TICKS_PER_MS` below: a millisecond is that many ticks.
const _: () = assert!(TIMEBASE_HZ / 1_000 == TICKS_PER_MS);

/// A counter reading in nanoseconds. Saturates rather than wrapping (at ~584 years), so
/// the result stays monotone whatever the counter does.
#[inline]
pub const fn ticks_to_ns(ticks: u64) -> u64 {
    ticks.saturating_mul(NS_PER_TICK)
}

/// Read the monotone clock in nanoseconds.
///
/// Reads the `time` CSR, **not** the CLINT: `mtime`'s MMIO page is not mapped in the
/// per-task page tables a syscall runs on, whereas the CSR needs no mapping. It is the
/// same counter. This is observation only -- nothing here arms `mtimecmp`, enables
/// `STIE` or touches `sie` (RFC-0.42-002 R3).
#[inline]
pub fn now_ns() -> u64 {
    // SAFETY: category=csr-asm `mcounteren.TM` is set in `m_mode_setup` before S-mode is entered, so the read cannot trap.
    ticks_to_ns(unsafe { super::csr::read_time() })
}

/// Read the current `mtime` counter.
///
/// # Safety
/// `CLINT_MTIME` must be a valid MMIO address (invariant on QEMU virt).
#[inline]
// SAFETY: category=mmio-access CLINT MMIO address is fixed at boot; access is serialised by the single hart context.
pub unsafe fn read_mtime() -> u64 {
    // SAFETY: category=mmio-access CLINT_MTIME is a valid read-only MMIO register on QEMU virt.
    // MMIO-ORDER: status_read
    unsafe { (CLINT_MTIME as *const u64).read_volatile() }
}

/// Set `mtimecmp` for hart 0 to schedule the next timer interrupt.
///
/// # Safety
/// `CLINT_MTIMECMP0` must be a valid MMIO address.
/// This write must happen in M-mode or from a context that has MMIO access.
#[inline]
// SAFETY: category=mmio-access CLINT MMIO address is fixed at boot; access is serialised by the single hart context.
pub unsafe fn set_mtimecmp(value: u64) {
    // MMIO-ORDER: device_kick
    // SAFETY: category=mmio-access CLINT_MTIMECMP0 is a valid writable MMIO register on QEMU virt.
    unsafe { (CLINT_MTIMECMP0 as *mut u64).write_volatile(value) };
}

/// Set the next timer interrupt to fire `TICK_INTERVAL` ticks from now.
///
/// # Safety
/// See `set_mtimecmp`.
// SAFETY: category=mmio-access CLINT MMIO address is fixed at boot; access is serialised by the single hart context.
pub unsafe fn schedule_next_tick() {
    // SAFETY: category=mmio-access both MMIO accesses are to valid CLINT registers.
    let now = unsafe { read_mtime() };
    // SAFETY: category=mmio-access CLINT MMIO address is fixed at boot; access is serialised by the single hart context.
    unsafe { set_mtimecmp(now + TICK_INTERVAL) };
}
