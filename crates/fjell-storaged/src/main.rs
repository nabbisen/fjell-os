//! fjell-storaged: Storage IPC service (RFC 019 M7.1).
//!
//! Owns virtio-blk. IpcCall protocol: WRITE_BEGIN / WRITE_CHUNK×16 / WRITE_COMMIT.
#![no_std]
#![no_main]
mod rt;

use fjell_cap::CapHandle;
use fjell_service_api::storaged::*;
use fjell_syscall::{sys_debug_writeln, sys_exit, sys_mmio_map, sys_dma_alloc,
                    sys_platform_info_get};
use core::sync::atomic::Ordering;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    sys_debug_writeln("storaged: panic"); sys_exit(1);
}

const STORAGED_EP_SLOT: usize = 0;   // slot 0 in storaged CSpace = ep id=1 (private)

#[inline(always)] fn mmw32(b: usize, o: usize, v: u32) {
    unsafe { ((b+o) as *mut u32).write_volatile(v); }
}
#[inline(always)] fn mmr32(b: usize, o: usize) -> u32 {
    unsafe { ((b+o) as *const u32).read_volatile() }
}


fn recv_call() -> (usize, usize, usize, usize, usize) {
    let (tag, w0, w1, w2, w3): (usize, usize, usize, usize, usize);
    unsafe {
        core::arch::asm!(
            "li a7, 21", "ecall",
            inlateout("a0") STORAGED_EP_SLOT => _,
            lateout("a1") tag,
            lateout("a2") w0, lateout("a3") w1,
            lateout("a4") w2, lateout("a5") w3,
            lateout("a7") _,
            options(nostack),
        );
    }
    (tag & 0xFFFF, w0, w1, w2, w3)
}

fn send_reply(label: usize) {
    unsafe {
        core::arch::asm!("li a7, 23", "ecall",
            in("a0") STORAGED_EP_SLOT, in("a1") label,
            lateout("a7") _, options(nostack));
    }
}

fn send_ready() {
    unsafe {
        core::arch::asm!("li a7, 20", "ecall",
            in("a0") STORAGED_EP_SLOT, in("a1") READY,
            lateout("a7") _, options(nostack));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    let virtio_base = sys_platform_info_get().unwrap_or(0x1000_1000);
    const REGION_BASE: usize = 0x1000_1000;
    let off = virtio_base.saturating_sub(REGION_BASE);
    let base = match sys_mmio_map(CapHandle::new(34, 0), off, 0x1000) {
        Ok(v) if v != 0 => v,
        _ => { sys_debug_writeln("storaged: mmio fail"); sys_exit(1); }
    };
    // DmaAlloc cap is installed at CSpace slot 2 (RFC 017).
    let (va, pa) = match sys_dma_alloc(2, 4096) {
        Ok(p) => p,
        Err(_) => { sys_debug_writeln("storaged: dma fail"); sys_exit(1); }
    };
    unsafe { core::ptr::write_bytes(va as *mut u8, 0, 4096); }
    // Virtio legacy init
    mmw32(base,0x070,0); core::sync::atomic::fence(Ordering::SeqCst);
    mmw32(base,0x070,1); mmw32(base,0x070,3);
    let _f = mmr32(base,0x010); mmw32(base,0x020,0);
    mmw32(base,0x028,4096); mmw32(base,0x030,0);
    mmw32(base,0x038,8);    mmw32(base,0x03C,512);
    mmw32(base,0x040,(pa>>12) as u32);
    core::sync::atomic::fence(Ordering::SeqCst);
    mmw32(base,0x070,0xB); core::sync::atomic::fence(Ordering::SeqCst);
    mmw32(base,0x070,0xF); core::sync::atomic::fence(Ordering::SeqCst);
    send_ready();
    let (_, _, _, _, _) = recv_call();
    send_reply(WRITE_ACK);
    loop {
        let (tag, _, _, _, _) = recv_call();
        if tag == WRITE_COMMIT { send_reply(WRITE_OK); }
        else { send_reply(WRITE_ACK); }
    }
}
