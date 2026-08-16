//! Minimal NS16550A-compatible UART driver.
//!
//! The QEMU `virt` machine exposes a 16550A-compatible UART at physical
//! address `0x1000_0000`.  This module provides a byte-level write interface
//! used by the early console.
//!
//! # Safety policy
//! All MMIO register accesses use `write_volatile` / `read_volatile` to
//! prevent the compiler from eliding or reordering hardware-visible writes.
//! The driver is single-threaded in M1 (no locking needed yet).

use core::fmt;

/// Physical base address of UART0 on QEMU `virt`.
const UART_BASE: usize = 0x1000_0000;

// NS16550A register offsets (byte-wide registers).
const UART_THR: usize = 0; // Transmitter Holding Register (write)
const UART_LCR: usize = 3; // Line Control Register
const UART_FCR: usize = 2; // FIFO Control Register

// RFC-0.25-001 D2: UART ownership is split by register, and the split is
// documented, not just implied. The kernel keeps THR and LSR bit 5 only; the
// `fjell-driver-uart` crate (userspace, its own register offsets) owns RBR,
// IER, and LSR bits 0-4.
//
// RBR (offset 0, read) and IER (offset 1) are listed here only because the
// kernel writes IER exactly once, at init, to enable the RX interrupt — see
// `enable_rx_interrupt` below. The kernel never reads RBR; that would be
// reading device data, which D1 reserves for the driver task.
const UART_IER: usize = 1; // Interrupt Enable Register
#[allow(dead_code)] // documents the offset the driver crate must also use
const UART_RBR: usize = 0; // Receiver Buffer Register (read) — driver-owned
#[allow(dead_code)] // documents the offset; kernel only ever reads bit 5
const UART_LSR: usize = 5; // Line Status Register

/// Handle to the UART peripheral.
///
/// Constructed as a ZST — the hardware address is a compile-time constant.
/// A `static mut` is used in `console.rs` only because `fmt::Write` requires
/// `&mut self`; see the SAFETY note there.
pub struct Uart;

impl Uart {
    /// Create a new (zero-sized) UART handle.
    pub const fn new() -> Self {
        Uart
    }

    /// Minimal UART initialisation.
    ///
    /// Sets 8-bit word length (LCR) and enables the receive/transmit FIFO
    /// (FCR).  No baud-rate divisor is set because QEMU `virt` operates at
    /// a virtual baud rate and ignores the divisor.
    ///
    /// # Safety
    /// Caller must ensure this runs exactly once, before any `putc` call,
    /// and that no other code accesses the UART MMIO region concurrently.
    pub fn init(&mut self) {
        let base = UART_BASE as *mut u8;
        // SAFETY: category=mmio-access UART_BASE is a valid MMIO address on QEMU virt.
        // Single-threaded boot context; no concurrent access possible.
        unsafe {
            // LCR = 0b0000_0011: 8-bit word length, 1 stop bit, no parity.
            // MMIO-ORDER: device_setup
            base.add(UART_LCR).write_volatile(0b0000_0011);
            // MMIO-ORDER: device_setup
            // FCR = 0b0000_0001: enable TX/RX FIFO.
            base.add(UART_FCR).write_volatile(0b0000_0001);
        }
    }

    /// Transmit a single byte over the UART.
    ///
    /// # Safety
    /// `init` must have been called once before the first `putc`.
    /// No concurrent callers.
    ///
    /// SAFETY invariant (RFC-0.25-001 D2, ownership split): this is the
    /// kernel's TX path. It writes THR and, if it ever reads LSR, must
    /// consult bit 5 (transmitter-empty) only — LSR bits 0-4 (data-ready,
    /// line errors) belong to `fjell-driver-uart`. Reading LSR clears its
    /// error/break bits, so acting on them here would race the driver's own
    /// RX-side read of the same register. `putc` does not currently poll
    /// LSR at all; this note exists so a future change that adds TX-empty
    /// polling does not reach past bit 5.
    pub fn putc(&mut self, byte: u8) {
        let base = UART_BASE as *mut u8;
        // SAFETY: category=mmio-access UART_BASE is a valid MMIO address on QEMU virt.
        // volatile write ensures the byte is not elided by the compiler.
        unsafe {
            // MMIO-ORDER: device_kick
            base.add(UART_THR).write_volatile(byte);
        }
    }

    /// Enable the UART's RX-data-ready interrupt (RFC-0.25-001 R5).
    ///
    /// Called exactly once, during kernel boot, after `init`. The kernel
    /// never touches `IER` again — D2 reserves ongoing IER management (and
    /// all of RBR, and LSR bits 0-4) to the driver task.
    ///
    /// # Safety
    /// Caller must ensure this runs exactly once, after `init`, before
    /// `sie.SEIE` is enabled (`arch::riscv64::csr::enable_interrupts`).
    pub fn enable_rx_interrupt(&mut self) {
        let base = UART_BASE as *mut u8;
        // SAFETY: category=mmio-access UART_BASE is a valid MMIO address on
        // QEMU virt; single boot hart, called once before interrupts are
        // enabled at the CPU level, so no concurrent access.
        unsafe {
            // IER bit 0 = "Received Data Available" interrupt enable.
            // MMIO-ORDER: device_setup
            base.add(UART_IER).write_volatile(0b0000_0001);
        }
    }
}

impl fmt::Write for Uart {
    /// Write a UTF-8 string slice to the UART one byte at a time.
    ///
    /// `\n` is automatically followed by `\r` to satisfy many terminal
    /// emulators that expect CRLF line endings on a raw serial port.
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            if byte == b'\n' {
                self.putc(b'\r');
            }
            self.putc(byte);
        }
        Ok(())
    }
}
