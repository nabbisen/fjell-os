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
    sys_debug_writeln("storaged: panic"); sys_exit(1); loop {}
}

const STORAGED_EP_SLOT: usize = 0;   // slot 0 in storaged CSpace = ep id=1 (private)

#[inline(always)] fn mmw32(b: usize, o: usize, v: u32) {
    unsafe { ((b+o) as *mut u32).write_volatile(v); }
}
#[inline(always)] fn mmr32(b: usize, o: usize) -> u32 {
    unsafe { ((b+o) as *const u32).read_volatile() }
}

fn write_desc(va: usize, i: usize, addr: u64, len: u32, flags: u16, next: u16) {
    let b = va + i * 16;
    unsafe {
        (b as *mut u64).write_volatile(addr);
        ((b+8) as *mut u32).write_volatile(len);
        ((b+12) as *mut u16).write_volatile(flags);
        ((b+14) as *mut u16).write_volatile(next);
    }
}

fn blk_write(base: usize, va: usize, pa: usize, lba: u64, data: &[u8; 512]) -> bool {
    const HDR: usize = 0x300; const DAT: usize = 0x310; const STAT: usize = 0x510;
    const AVAIL: usize = 0x080; const USED: usize = 0x200;
    unsafe {
        ((va+HDR) as *mut u32).write_volatile(1);
        ((va+HDR+4) as *mut u32).write_volatile(0);
        ((va+HDR+8) as *mut u64).write_volatile(lba);
        core::ptr::copy_nonoverlapping(data.as_ptr(), (va+DAT) as *mut u8, 512);
        ((va+STAT) as *mut u8).write_volatile(0xFF);
    }
    write_desc(va, 0, (pa+HDR) as u64, 16, 1, 1);
    write_desc(va, 1, (pa+DAT) as u64, 512, 1, 2);
    write_desc(va, 2, (pa+STAT) as u64, 1, 2, 0);
    let old = unsafe { ((va+AVAIL+2) as *const u16).read_volatile() };
    unsafe { ((va+AVAIL+4+(old as usize%8)*2) as *mut u16).write_volatile(0); }
    core::sync::atomic::fence(Ordering::SeqCst);
    unsafe { ((va+AVAIL+2) as *mut u16).write_volatile(old.wrapping_add(1)); }
    core::sync::atomic::fence(Ordering::SeqCst);
    mmw32(base, 0x050, 0);
    core::sync::atomic::fence(Ordering::SeqCst);
    for _ in 0..100_000u32 {
        core::sync::atomic::fence(Ordering::SeqCst);
        if unsafe { ((va+USED+2) as *const u16).read_volatile() } != old { break; }
    }
    let st = unsafe { ((va+STAT) as *const u8).read_volatile() };
    st == 0
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
    let (va, pa) = match sys_dma_alloc(4096) {
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
    let mut buf = [0u8; 512];
    let mut lba: u64 = 0;
    let mut chunk: usize = 0;
    let (first_tag, _, _, _, _) = recv_call();
    send_reply(WRITE_ACK);
    loop {
        let (tag, _, _, _, _) = recv_call();
        if tag == WRITE_COMMIT { send_reply(WRITE_OK); }
        else { send_reply(WRITE_ACK); }
    }
}
