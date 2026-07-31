#![allow(unused_assignments)] // IPC polling idiom: t/w* are overwritten by sys_ipc_recv
#![no_std]
#![no_main]
mod rt;
use fjell_proxy_text as _;
use fjell_service_api::proxy_text as proto;
use fjell_syscall::sys_debug_writeln;

const EP_SLOT: u32 = 0;

fn send_ready() {
    // SAFETY: category=raw-pointer-deref IPC call slot is valid; response buffer length is bounded by MAX_IPC_MSG.
    unsafe {
        core::arch::asm!("li a7, 20","ecall", in("a0") EP_SLOT as usize, in("a1") proto::READY, lateout("a0") _, lateout("a7") _, options(nostack));
    }
}

fn recv_call() -> (usize, usize, usize, usize, usize) {
    let (mut t, mut w0, mut w1, mut w2, mut w3) = (0usize, 0usize, 0usize, 0usize, 0usize);
    // SAFETY: category=raw-pointer-deref IPC call slot is valid; response buffer length is bounded by MAX_IPC_MSG.
    unsafe {
        core::arch::asm!("li a7, 21","ecall", in("a0") EP_SLOT as usize, lateout("a1") t, lateout("a2") w0, lateout("a3") w1, lateout("a4") w2, lateout("a5") w3, lateout("a7") _, options(nostack));
    }
    (t, w0, w1, w2, w3)
}

fn reply(tag: usize, w0: usize, w1: usize, w2: usize) {
    // SAFETY: category=raw-pointer-deref IPC call slot is valid; response buffer length is bounded by MAX_IPC_MSG.
    unsafe {
        core::arch::asm!("li a7, 23","ecall", in("a0") 0usize, in("a1") tag, in("a2") w0, in("a3") w1, in("a4") w2, lateout("a7") _, options(nostack));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    send_ready();
    sys_debug_writeln("M5: proxy-text started");
    loop {
        let (tag_packed, _w0, _w1, _w2, _w3) = recv_call();
        let tag = tag_packed & 0xFFFF;
        match tag {
            // RFC-v0.23-001 Slice 2 adds real handling for RENDER_* here.
            _ => reply(proto::ERR, 0, 0, 0),
        }
    }
}
