//! Fjell OS Kernel — M1: Bootable Kernel
//!
//! Entry point after the assembly boot shim sets up BSS and the stack.
//! Initialises the UART and prints a startup banner, then spins.
//!
//! Future milestones extend `kmain` progressively:
//!   M2 — `kmain(hart_id: usize, dtb_pa: usize)`, memory + task init
//!   M3 — IPC, capability table

#![no_std]
#![no_main]

mod boot;
mod console;
mod uart;

use core::panic::PanicInfo;

/// Kernel entry point.
///
/// Called by `_start` in `boot.rs` after hart selection, BSS clear, and
/// stack pointer initialisation.  The signature will be extended in M2 to
/// accept `hart_id` and `dtb_pa` passed from the M-mode shim.
///
/// # Safety
/// Invoked exactly once from assembly with a valid stack pointer.
/// BSS has been zeroed by the time this function executes.
#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    // Initialise the UART before any console output.
    //
    // SAFETY: called once on the single boot hart before any println! use.
    unsafe {
        console::init();
    }

    println!("=============================");
    println!("  Fjell OS kernel started.   ");
    println!("=============================");
    println!();
    println!("arch  : riscv64");
    println!("mach  : qemu-virt");
    println!("stage : M1 bootable kernel");

    loop {}
}

/// Kernel panic handler.
///
/// Writes the panic message to the UART and halts.  In M2 this will also
/// dump the faulting hart context.
///
/// # SAFETY invariants
/// - The function never returns (`-> !`).
/// - Uses `console::_print` which accesses UART via raw pointer — sound
///   because we are in a panic (single path of execution, no concurrency).
/// - If the panic fires before `console::init()`, the UART register writes
///   are still safe MMIO operations; output may be garbled but we halt safely.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // `println!` calls `_print` which goes through the raw-pointer UART path.
    // No `unsafe` block needed here because `_print` encapsulates it.
    println!("\n[KERNEL PANIC] {}", info);
    loop {}
}
