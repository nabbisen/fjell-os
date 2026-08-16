//! driver-uart — user-space UART RX driver for Fjell OS (RFC-0.25-001).
//!
//! Binds UART0's IRQ line, waits for it, reads one byte from `RBR`, and
//! delivers it over IPC to init. Stops at "bytes to a service" — no shell,
//! no echo, no line editing (RFC-0.25-001 non-goals; the next line's
//! decisions).
#![no_std]
#![no_main]
mod rt;

use fjell_cap::CapHandle;
use fjell_syscall::{
    sys_debug_writeln, sys_exit, sys_ipc_try_send, sys_irq_ack, sys_irq_bind, sys_irq_wait,
    sys_mmio_map,
};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    sys_debug_writeln("driver-uart: panic");
    sys_exit(1);
}

// ── Cap slot indices ──────────────────────────────────────────────────────────
//
// The kernel installs these directly at spawn time
// (`crates/fjell-kernel/src/task/spawn.rs`) — driver-uart is a bootstrap
// exception, like driver-virtio-net, devmgr, and storaged: it cannot yet use
// cap-broker, since `sys_cap_install` has no dispatch arm (RFC-0.25-001 D4).
//   slot 0  — Endpoint    (init's uart-rx endpoint; driver posts received bytes here)
//   slot 1  — Interrupt   (UART0's IRQ line, irq 10)
//   slot 32 — MmioRegion  (UART0 device registers, region 1; slot number
//                          matches the kernel's `31 + region_idx` convention)
const CAP_INIT_EP: CapHandle = CapHandle(0);
const CAP_IRQ: CapHandle = CapHandle(1);
const CAP_MMIO: CapHandle = CapHandle(32);

// NS16550A register offsets — the driver-owned half of the D2 split
// (RFC-0.25-001). The kernel keeps `THR` and `LSR` bit 5 only; this driver
// owns `RBR`, `IER`, and `LSR` bits 0-4. Reading `LSR` clears its error and
// break bits, so this driver must never act on bit 5 (transmitter-empty) —
// that belongs to the kernel's TX path (`crates/fjell-kernel/src/uart.rs`).
const UART_RBR: usize = 0;
const UART_LSR: usize = 5;
const LSR_DATA_READY: u8 = 0b0000_0001;

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("driver-uart: starting");

    let device_va = match sys_mmio_map(CAP_MMIO, 0, 0x1000) {
        Ok(va) => va,
        Err(_) => {
            sys_debug_writeln("driver-uart: mmio map failed");
            sys_exit(1);
        }
    };
    sys_debug_writeln("driver-uart: mmio mapped");

    match sys_irq_bind(CAP_IRQ) {
        Ok(()) => sys_debug_writeln("driver-uart: IRQ bound"),
        Err(_) => {
            sys_debug_writeln("driver-uart: IRQ bind failed");
            sys_exit(1);
        }
    }

    sys_debug_writeln("driver-uart: ready");

    loop {
        match sys_irq_wait(CAP_IRQ) {
            Ok(()) => {
                // SAFETY: category=mmio-access `device_va` was returned by
                // `sys_mmio_map` for this task's own UART0 `MmioRegion`
                // capability. This driver owns RBR and LSR bits 0-4 (D2);
                // it never touches LSR bit 5, which belongs to the kernel's
                // TX path.
                // MMIO-ORDER: status_read
                let lsr = unsafe { (device_va as *const u8).add(UART_LSR).read_volatile() };
                if lsr & LSR_DATA_READY != 0 {
                    // SAFETY: category=mmio-access same `device_va` and same
                    // ownership rationale as the LSR read above; RBR is
                    // driver-owned under D2.
                    // MMIO-ORDER: status_read
                    let byte = unsafe { (device_va as *const u8).add(UART_RBR).read_volatile() };
                    sys_debug_writeln("driver-uart: byte received");
                    // The endpoint carries exactly one message shape (a
                    // received byte), so the raw label IS the byte value —
                    // no separate tag namespace needed.
                    let _ = sys_ipc_try_send(CAP_INIT_EP.0, byte as usize);
                }
                sys_irq_ack(CAP_IRQ).unwrap_or_default();
            }
            Err(_) => {
                sys_debug_writeln("driver-uart: irq_wait error");
                sys_exit(1);
            }
        }
    }
}
