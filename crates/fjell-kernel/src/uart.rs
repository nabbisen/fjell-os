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
        // SAFETY: UART_BASE is a valid MMIO address on QEMU virt.
        // Single-threaded boot context; no concurrent access possible.
        unsafe {
            // LCR = 0b0000_0011: 8-bit word length, 1 stop bit, no parity.
            base.add(UART_LCR).write_volatile(0b0000_0011);
            // FCR = 0b0000_0001: enable TX/RX FIFO.
            base.add(UART_FCR).write_volatile(0b0000_0001);
        }
    }

    /// Transmit a single byte over the UART.
    ///
    /// # Safety
    /// `init` must have been called once before the first `putc`.
    /// No concurrent callers.
    pub fn putc(&mut self, byte: u8) {
        let base = UART_BASE as *mut u8;
        // SAFETY: UART_BASE is a valid MMIO address on QEMU virt.
        // volatile write ensures the byte is not elided by the compiler.
        unsafe {
            base.add(UART_THR).write_volatile(byte);
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
