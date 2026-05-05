//! Early kernel console backed by the UART driver.
//!
//! Provides `print!` and `println!` macros for use before a proper locking
//! mechanism is available.
//!
//! # Temporary design note (M1)
//! `UART` is stored in a `static UnsafeCell` because `fmt::Write` requires
//! `&mut self` and the kernel is strictly single-threaded at this stage.
//! Edition 2024 denies mutable references to `static mut`, so we use
//! `UnsafeCell` + raw pointer dereference instead — which has identical
//! runtime semantics but makes the unsafety explicit at each call site.
//! In M2+ this will be replaced by a spinlock-protected console object.

use crate::uart::Uart;
use core::cell::UnsafeCell;
use core::fmt::{self, Write};

/// Newtype wrapper so we can implement `Sync` for a `static UnsafeCell<Uart>`.
///
/// # SAFETY
/// `Uart` is only accessed from the single boot hart in M1.
/// `Sync` is required to place the value in a `static`.
struct SyncUnsafeCell(UnsafeCell<Uart>);

// SAFETY: M1 invariant — accessed exclusively from hart 0, no concurrency.
// TODO(M2+): remove once a spinlock wrapper is introduced.
unsafe impl Sync for SyncUnsafeCell {}

/// Global UART instance, wrapped in `UnsafeCell` to allow interior mutability
/// from a shared `static` without triggering edition-2024 `static_mut_refs`.
static UART: SyncUnsafeCell = SyncUnsafeCell(UnsafeCell::new(Uart::new()));

/// Initialise the UART hardware.
///
/// Must be called exactly once, before any `print!` / `println!` use.
///
/// # Safety
/// Caller must guarantee single-threaded context and that `init` has not
/// already been called.
pub unsafe fn init() {
    // SAFETY: single boot hart; `init` called exactly once before any print.
    unsafe { (*UART.0.get()).init() }
}

/// Internal print implementation called by the `print!` macro.
pub fn _print(args: fmt::Arguments) {
    // SAFETY: M1 invariant — single hart, no concurrent callers.
    // The raw pointer dereference is sound because:
    //   - `UART.0.get()` always returns a valid, aligned pointer.
    //   - No other reference to `*UART.0.get()` exists concurrently.
    // TODO(M2+): replace with a spinlock-protected write.
    unsafe {
        (*UART.0.get()).write_fmt(args).unwrap();
    }
}

/// Print without a trailing newline.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::console::_print(format_args!($($arg)*))
    };
}

/// Print with a trailing newline.
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => {
        $crate::print!("{}\n", format_args!($($arg)*))
    };
}
