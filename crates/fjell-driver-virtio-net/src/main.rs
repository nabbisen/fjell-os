//! driver-virtio-net — User-space virtio-mmio network driver for Fjell OS.
//!
//! v0.4.0-alpha.1: Driver skeleton.  Receives caps from `devmgr`, binds the
//! IRQ, negotiates features, emits `NET_DRIVER_READY`, then enters an IRQ-wait
//! loop using `sys_irq_wait` / `sys_irq_ack` (RFC v0.4-001 §7.3).
//!
//! Full virtio register reads and ring ops land in v0.4.0-alpha.2.
#![no_std]
#![no_main]
mod rt;

use fjell_syscall::{sys_debug_writeln, sys_exit, sys_irq_wait, sys_irq_ack};
use fjell_cap::CapHandle;
use fjell_net_format::{
    NetDeviceDescriptor, NetDeviceState, NetIpcTag,
};

// Re-use the host-testable core types.
use fjell_driver_virtio_net::{
    DriverStateBlock, DriverState, Ring,
    negotiate_features, VirtioFeatureFlags,
};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    sys_debug_writeln("driver-virtio-net: panic");
    sys_exit(1);
}

// ── Cap slot indices ──────────────────────────────────────────────────────────
//
// devmgr installs capabilities into these CSpace slots before starting
// the driver:
//   slot 0 — MmioRegion  (device registers)
//   slot 1 — DmaRegion   (RX ring)
//   slot 2 — DmaRegion   (TX ring)
//   slot 3 — Interrupt   (IRQ line)
//   slot 4 — Endpoint    (netd send-end; driver posts RX events here)
//   slot 5 — Endpoint    (service-manager ready endpoint)
//
const CAP_MMIO:    CapHandle = CapHandle(0);
const CAP_DMA_RX:  CapHandle = CapHandle(1);
const CAP_DMA_TX:  CapHandle = CapHandle(2);
const CAP_IRQ:     CapHandle = CapHandle(3);
const CAP_NETD_EP: CapHandle = CapHandle(4);
const CAP_SMGR_EP: CapHandle = CapHandle(5);

// ── IPC helpers ─────────────────────────────────────────────────────────────

fn ipc_send_tag(ep: CapHandle, tag: u16) {
    unsafe {
        core::arch::asm!(
            "li a7, 20", "ecall",
            in("a0") ep.0 as usize, in("a1") tag as usize,
            lateout("a0") _, lateout("a7") _, options(nostack)
        );
    }
}

// ── Driver entry ─────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("driver-virtio-net: starting");

    let mut state = DriverStateBlock::new();
    let mut rx_ring = Ring::new();
    let mut tx_ring = Ring::new();

    // ── Init phase ───────────────────────────────────────────────────────────
    state.transition(DriverState::Init).unwrap_or_else(|_| {
        sys_debug_writeln("driver-virtio-net: state transition failed");
        sys_exit(1);
    });

    // Read device descriptor from static QEMU defaults (devmgr will provide
    // via IPC in v0.4.0-alpha.2; for now use the known QEMU virt parameters).
    let dev = NetDeviceDescriptor::QEMU_VIRT_DEFAULT;
    sys_debug_writeln("driver-virtio-net: device descriptor loaded");

    // Feature negotiation (against a simulated minimal offered set for now).
    let offered = VirtioFeatureFlags(
        fjell_driver_virtio_net::VIRTIO_NET_F_MAC
        | fjell_driver_virtio_net::VIRTIO_NET_F_STATUS,
    );
    let (negotiated, legacy) = negotiate_features(offered);
    if legacy {
        sys_debug_writeln("driver-virtio-net: legacy mode (no VIRTIO_F_VERSION_1)");
    }
    sys_debug_writeln("driver-virtio-net: features negotiated");

    // Bind IRQ (blocking bind; real kernel call in cross-compiled binary).
    match fjell_syscall::sys_irq_bind(CAP_IRQ) {
        Ok(()) => sys_debug_writeln("driver-virtio-net: IRQ bound"),
        Err(_) => {
            sys_debug_writeln("driver-virtio-net: IRQ bind failed");
            sys_exit(1);
        }
    }

    // Transition to Ready.
    state.transition(DriverState::Ready).unwrap_or_else(|_| sys_exit(1));
    sys_debug_writeln("driver-virtio-net: ready");

    // Notify service-manager.
    ipc_send_tag(CAP_SMGR_EP, NetIpcTag::DriverReady as u16);
    sys_debug_writeln("driver-virtio-net: sent NET_DRIVER_READY");

    // ── IRQ wait loop ────────────────────────────────────────────────────────
    loop {
        match sys_irq_wait(CAP_IRQ) {
            Ok(()) => {
                state.transition(DriverState::HandleRx).unwrap_or_else(|_| {
                    state.fault();
                });

                if !state.is_faulted() {
                    // Process RX ring entries — ring polling logic lands in alpha.2.
                    ipc_send_tag(CAP_NETD_EP, NetIpcTag::PacketRx as u16);
                    state.transition(DriverState::Ready).unwrap_or_default();
                }

                sys_irq_ack(CAP_IRQ).unwrap_or_default();
            }
            Err(_) => {
                sys_debug_writeln("driver-virtio-net: irq_wait error; entering faulted");
                state.fault();
                ipc_send_tag(CAP_NETD_EP, NetIpcTag::DeviceRevoked as u16);
                sys_exit(1);
            }
        }
    }
}
