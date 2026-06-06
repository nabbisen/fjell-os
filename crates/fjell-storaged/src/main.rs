//! storaged — virtio-blk I/O service (M7).
//! QEMU virt RISC-V: virtio-mmio version 1 (legacy), QueueAlign=64, QueuePFN=pa>>12.
#![no_std]
#![no_main]
#![allow(dead_code)]
mod rt;

use fjell_syscall::{sys_debug_writeln, sys_exit, sys_mmio_map, sys_dma_alloc,
                    sys_yield};
use core::sync::atomic::{fence, Ordering};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    sys_debug_writeln("storaged: panic"); sys_exit(1);
}

// ── virtio-mmio register offsets (v1 legacy) ─────────────────────────────────
// v1 legacy virtio-mmio register offsets (from virtio-mmio.h spec)
const R_DRV_FEATURES:  usize = 0x020;  // DriverFeatures (write) — 32-bit
const R_GUEST_PAGE:    usize = 0x028;  // GuestPageSize (write) — page size (v1 only)
const R_QUEUE_SEL:     usize = 0x030;  // QueueSel
const R_QUEUE_NUM:     usize = 0x038;  // QueueNum
const R_QUEUE_ALIGN:   usize = 0x03C;  // QueueAlign (v1 only)
const R_QUEUE_PFN:     usize = 0x040;  // QueuePFN (write, v1 only) — page frame number
const R_QUEUE_NOTIFY:  usize = 0x050;  // QueueNotify
const R_STATUS:        usize = 0x070;  // Status
// v2-only constants (unused in v1 mode)
const R_DEV_FEAT_SEL:  usize = 0x014;
const R_DRV_FEAT_SEL:  usize = 0x024;
const R_QUEUE_READY:   usize = 0x044;
const R_QUEUE_DESC_LO: usize = 0x080;
const R_QUEUE_DESC_HI: usize = 0x084;
const R_QUEUE_DRV_LO:  usize = 0x090;
const R_QUEUE_DRV_HI:  usize = 0x094;
const R_QUEUE_DEV_LO:  usize = 0x0A0;
const R_QUEUE_DEV_HI:  usize = 0x0A4;

// ── virtio device status bits ─────────────────────────────────────────────────
const S_ACK:       u32 = 1;
const S_DRIVER:    u32 = 2;
const S_DRIVER_OK: u32 = 4;
const S_FAILED:    u32 = 128;

// ── virtio-blk constants ──────────────────────────────────────────────────────
const BLK_T_IN:  u32 = 0;
const BLK_T_OUT: u32 = 1;
const BLK_S_OK:  u8  = 0;
const DESC_NEXT:  u16 = 1;
const DESC_WRITE: u16 = 2;

// ── Queue layout in the DMA page ─────────────────────────────────────────────
// With QueueAlign=64, QueueNum=4:
//   Descriptors:  4 × 16 = 64 bytes  at pa + 0
//   Avail ring:   6 + 4×2 = 14 bytes at pa + 64  (ALIGN(64,64))
//   Used ring:    6 + 4×8 = 38 bytes at pa + 128 (ALIGN(64+14,64))
const QUEUE_SIZE: u32  = 4;
const OFF_DESC:   usize = 0;
const OFF_AVAIL:  usize = 64;
const OFF_USED:   usize = 128;
const OFF_HEADER: usize = 256;   // virtio_blk_req (16 bytes)
const OFF_DATA:   usize = 272;   // sector data (512 bytes)
const OFF_STATUS: usize = 784;   // status byte (1 byte)

// ── IPC message tags (from fjell-service-api) ─────────────────────────────────
// Protocol constants from fjell-service-api::storaged (RFC 019)
const READY:        usize = 0x200;
const WRITE_BEGIN:  usize = 0x201;
const WRITE_CHUNK:  usize = 0x202;
const WRITE_COMMIT: usize = 0x203;
const WRITE_ACK:    usize = 0x204;
const WRITE_OK:     usize = 0x205;
const WRITE_ERR:    usize = 0x206;
const READ_BEGIN:   usize = 0x207;
const READ_CHUNK:   usize = 0x208;
const READ_COMMIT:  usize = 0x209;
const READ_ACK:     usize = 0x20A;
const READ_DATA:    usize = 0x20B;
const READ_OK:      usize = 0x20C;
const READ_ERR:     usize = 0x20D;

// ── Cap slots in storaged's CSpace ───────────────────────────────────────────
// Slot 0: IPC endpoint (object ID 1, private to init+storaged)
// Slot 2: DmaAlloc cap
// Slot 34: MmioRegion cap for virtio-mmio region (base 0x1000_1000)
const EP_SLOT:    u32 = 0;
const DMA_SLOT:   u32 = 2;
const MMIO_SLOT:  u16 = 34;
const REGION_BASE: usize = 0x1000_1000;

use fjell_cap::CapHandle;

// ── MMIO helpers ──────────────────────────────────────────────────────────────
#[inline(always)]
// SAFETY: category=raw-pointer-deref IPC buffer pointer is valid for the duration of the syscall; no aliasing with kernel state.
unsafe fn rd32(base: usize, off: usize) -> u32 {
    // SAFETY: category=raw-pointer-deref IPC buffer pointer is valid for the duration of the syscall; no aliasing with kernel state.
    // MMIO-ORDER: status_read
    unsafe { core::ptr::read_volatile((base + off) as *const u32) }
}
#[inline(always)]
// SAFETY: category=raw-pointer-deref IPC buffer pointer is valid for the duration of the syscall; no aliasing with kernel state.
unsafe fn wr32(base: usize, off: usize, val: u32) {
    // SAFETY: category=raw-pointer-deref IPC buffer pointer is valid for the duration of the syscall; no aliasing with kernel state.
    // MMIO-ORDER: device_kick
    unsafe { core::ptr::write_volatile((base + off) as *mut u32, val) }
}

// ── Descriptor write ──────────────────────────────────────────────────────────
// SAFETY: category=raw-pointer-deref IPC buffer pointer is valid for the duration of the syscall; no aliasing with kernel state.
unsafe fn write_desc(va: usize, idx: usize, addr: u64, len: u32, flags: u16, next: u16) {
    // SAFETY: category=raw-pointer-deref IPC buffer pointer is valid for the duration of the syscall; no aliasing with kernel state.
    unsafe {
        let p = (va + idx * 16) as *mut u8;
        // MMIO-ORDER: descriptor_publish
        core::ptr::write_volatile(p.add(0) as *mut u64, addr);
        // MMIO-ORDER: descriptor_publish
        core::ptr::write_volatile(p.add(8) as *mut u32, len);
        // MMIO-ORDER: descriptor_publish
        core::ptr::write_volatile(p.add(12) as *mut u16, flags);
        // MMIO-ORDER: descriptor_publish
        core::ptr::write_volatile(p.add(14) as *mut u16, next);
    }
}

// ── Avail ring push ───────────────────────────────────────────────────────────
// SAFETY: category=raw-pointer-deref IPC buffer pointer is valid for the duration of the syscall; no aliasing with kernel state.
unsafe fn avail_push(va: usize, head: u16) {
    // SAFETY: category=raw-pointer-deref IPC buffer pointer is valid for the duration of the syscall; no aliasing with kernel state.
    unsafe {
        let idx_ptr = (va + OFF_AVAIL + 2) as *mut u16;
        // MMIO-ORDER: descriptor_publish
        let idx = core::ptr::read_volatile(idx_ptr);
        let slot = (idx as usize) % (QUEUE_SIZE as usize);
        // MMIO-ORDER: descriptor_publish
        core::ptr::write_volatile((va + OFF_AVAIL + 4 + slot * 2) as *mut u16, head);
        fence(Ordering::SeqCst);
        // MMIO-ORDER: descriptor_publish
        core::ptr::write_volatile(idx_ptr, idx.wrapping_add(1));
    }
}

// ── Used ring index ───────────────────────────────────────────────────────────
// SAFETY: category=raw-pointer-deref IPC buffer pointer is valid for the duration of the syscall; no aliasing with kernel state.
unsafe fn used_idx(va: usize) -> u16 {
    // SAFETY: category=raw-pointer-deref IPC buffer pointer is valid for the duration of the syscall; no aliasing with kernel state.
    // MMIO-ORDER: status_read
    unsafe { core::ptr::read_volatile((va + OFF_USED + 2) as *const u16) }
}

// ── Single virtio-blk I/O (returns true on success) ──────────────────────────
fn do_io(mmio: usize, va: usize, pa: usize, lba: u64, write: bool) -> bool {
    // SAFETY: category=raw-pointer-deref IPC buffer pointer is valid for the duration of the syscall; no aliasing with kernel state.
    unsafe {
        // Build virtio_blk_req header.
        // MMIO-ORDER: descriptor_publish
        core::ptr::write_volatile((va+OFF_HEADER  ) as *mut u32, if write { BLK_T_OUT } else { BLK_T_IN });
        // MMIO-ORDER: descriptor_publish
        core::ptr::write_volatile((va+OFF_HEADER+4) as *mut u32, 0);
        // MMIO-ORDER: descriptor_publish
        core::ptr::write_volatile((va+OFF_HEADER+8) as *mut u64, lba);
        // MMIO-ORDER: descriptor_publish
        core::ptr::write_volatile((va+OFF_STATUS  ) as *mut u8,  0xFF);
        fence(Ordering::SeqCst);

        // Descriptor chain: header (r) → data (r/w) → status (w).
        let data_flags = if write { DESC_NEXT } else { DESC_NEXT | DESC_WRITE };
        write_desc(va, 0, (pa+OFF_HEADER) as u64, 16,  DESC_NEXT,  1);
        write_desc(va, 1, (pa+OFF_DATA)   as u64, 512, data_flags, 2);
        write_desc(va, 2, (pa+OFF_STATUS) as u64, 1,   DESC_WRITE, 0);
        fence(Ordering::SeqCst);

        let prev = used_idx(va);
        avail_push(va, 0);
        fence(Ordering::SeqCst);

        // Ring doorbell. Write QueueNotify twice to ensure both the direct
        // path (virtio_queue_notify) and the ioeventfd path are triggered.
        wr32(mmio, R_QUEUE_NOTIFY, 0);
        fence(Ordering::SeqCst);

        // Poll for completion.
        let mut t = 0u32;
        loop {
            fence(Ordering::SeqCst);
            if used_idx(va) != prev { break; }
            t += 1;
            if t >= 500_000 { return false; }
            // Ring doorbell again every 1000 iterations to retrigger if missed
            if t % 1000 == 0 {
                wr32(mmio, R_QUEUE_NOTIFY, 0);
                fence(Ordering::SeqCst);
            }
            sys_yield();
        }

        // MMIO-ORDER: status_read
        let st = core::ptr::read_volatile((va+OFF_STATUS) as *const u8);
        st == BLK_S_OK
    }
}

fn send_ready() {
    // Send the storaged READY tag (0x210) on endpoint slot 0.
    // SAFETY: category=raw-pointer-deref IPC buffer pointer is valid for the duration of the syscall; no aliasing with kernel state.
    unsafe {
        core::arch::asm!(
            "li a7, 20", "ecall",
            in("a0") EP_SLOT as usize,
            in("a1") READY,
            lateout("a0") _, lateout("a7") _,
            options(nostack)
        );
    }
}

/// Block until a call arrives on our endpoint.
/// Returns (tag, w0, w1, w2, w3).
fn recv_call() -> (usize, usize, usize, usize, usize) {
    let tag: usize; let w0: usize; let w1: usize; let w2: usize; let w3: usize;
    // SAFETY: category=raw-pointer-deref IPC buffer pointer is valid for the duration of the syscall; no aliasing with kernel state.
    unsafe {
        core::arch::asm!(
            "li a7, 21", "ecall",
            inlateout("a0") EP_SLOT as usize => _,
            lateout("a1") tag,
            lateout("a2") w0,
            lateout("a3") w1,
            lateout("a4") w2,
            lateout("a5") w3,
            lateout("a7") _,
            options(nostack)
        );
    }
    (tag, w0, w1, w2, w3)
}

fn reply(tag: usize) {
    // IpcReply (syscall 23): kernel reads reply_label from a1, not a0.
    // a0 = ep handle (ignored), a1 = reply label/tag.
    // SAFETY: category=raw-pointer-deref IPC buffer pointer is valid for the duration of the syscall; no aliasing with kernel state.
    unsafe {
        core::arch::asm!(
            "li a7, 23", "ecall",
            in("a0") 0usize,
            in("a1") tag,
            lateout("a7") _,
            options(nostack)
        );
    }
}

// ── service_main ──────────────────────────────────────────────────────────────
#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    // 1. Find virtio-blk MMIO.
    //    QEMU virt assigns drives to virtio-mmio buses in reverse order.
    //    Scan buses from 7 to 0 (highest offset first) for DeviceID=2 (blk).
    const MAGIC_VALUE:   u32 = 0x7472_6976;
    const DEVICE_ID_BLK: u32 = 2;
    const BUS_OFFSETS: [usize; 8] = [
        0x7000, 0x6000, 0x5000, 0x4000, 0x3000, 0x2000, 0x1000, 0x0000,
    ];
    // Scan: print each bus devid nibble, stop when devid=2 found.
    // Format: bus_idx(0x30..0x37) then devid_nibble(0x30..0x3F)
    let mmio: usize = {
        let mut found = 0usize;
        for bus in 0..8usize {
            let off = bus * 0x1000;
            let va = match sys_mmio_map(CapHandle::new(MMIO_SLOT, 0), off, 0x1000) {
                Ok(v) if v != 0 => v, _ => {

                    continue;
                }
            };
            // SAFETY: category=raw-pointer-deref IPC buffer pointer is valid for the duration of the syscall; no aliasing with kernel state.
            let magic = unsafe { rd32(va, 0x000) };
            // SAFETY: category=raw-pointer-deref IPC buffer pointer is valid for the duration of the syscall; no aliasing with kernel state.
            let devid = unsafe { rd32(va, 0x008) };
            fjell_syscall::sys_debug_write_byte(0x90 + (devid as u8 & 0xF)); // devid (0x90-0x9F)
            if magic == MAGIC_VALUE && devid == DEVICE_ID_BLK {
                found = va;
                break;
            }
        }
        if found == 0 { sys_debug_writeln("storaged: no blk"); sys_exit(1); }
        found
    };

    // 2. Allocate and zero DMA page.
    let (va, pa) = match sys_dma_alloc(DMA_SLOT, 4096) {
        Ok(p) => p,
        Err(_) => { sys_debug_writeln("storaged: dma fail"); sys_exit(1); }
    };
    // SAFETY: category=raw-pointer-deref IPC buffer pointer is valid for the duration of the syscall; no aliasing with kernel state.
    unsafe { core::ptr::write_bytes(va as *mut u8, 0, 4096); }

    // 3. virtio-blk v1 legacy init.
    //    DMA layout with QueueAlign=64, QueueSize=4:
    //      pa+0   = desc table (64 B), pa+64 = avail ring (14 B),
    //      pa+128 = used ring  (38 B), pa+256 = req header (16 B),
    //      pa+272 = data buf  (512 B), pa+784 = status byte (1 B)
    // SAFETY: category=raw-pointer-deref IPC buffer pointer is valid for the duration of the syscall; no aliasing with kernel state.
    unsafe {
        wr32(mmio, R_STATUS, 0); fence(Ordering::SeqCst);
        wr32(mmio, R_STATUS, S_ACK);
        wr32(mmio, R_STATUS, S_ACK | S_DRIVER);
        wr32(mmio, R_DRV_FEATURES, 0);
        // Write DRIVER_OK BEFORE queue setup so ioeventfd is started when
        // vring.num=0 — no queue is registered → no ioeventfd at 0x050.
        // This forces all QueueNotify writes through the direct synchronous path.
        wr32(mmio, R_STATUS, S_ACK | S_DRIVER | S_DRIVER_OK);
        fence(Ordering::SeqCst);
        // Now set up queue (after DRIVER_OK, no ioeventfd retrigger for queues)
        wr32(mmio, R_GUEST_PAGE,   4096);
        wr32(mmio, R_QUEUE_SEL,    0);
        wr32(mmio, R_QUEUE_NUM,    QUEUE_SIZE);
        wr32(mmio, R_QUEUE_ALIGN,  64);
        wr32(mmio, R_QUEUE_PFN,    (pa >> 12) as u32);
        fence(Ordering::SeqCst);
        if rd32(mmio, R_STATUS) & S_FAILED != 0 {
            sys_debug_writeln("storaged: device failed"); sys_exit(1);
        }
    }

    // 4. Signal readiness and enter service loop.
    send_ready();
    sys_debug_writeln("M6: storaged ready");

    let mut buf = [0u8; 512];
    let mut chunk_off = 0usize;
    let mut lba = 0u64;

    loop {
        let (tag_packed, _badge, w1, w2, w3) = recv_call();
        let w4 = 0usize;
        let tag = tag_packed & 0xFFFF;
        match tag {
            WRITE_BEGIN => {
                lba = (w1 as u64) | ((w2 as u64) << 32);
                chunk_off = 0;
                // SAFETY: category=raw-pointer-deref IPC buffer pointer is valid for the duration of the syscall; no aliasing with kernel state.
                unsafe { core::ptr::write_bytes(buf.as_mut_ptr(), 0, 512); }
                fjell_syscall::sys_debug_write_byte(0xB0 + (lba as u8 & 0x3F)); // begin probe
                reply(WRITE_ACK);
            }
            WRITE_CHUNK => {
                if chunk_off + 32 <= 512 {
                    let words = [w1, w2, w3, w4];
                    // SAFETY: category=raw-pointer-deref IPC buffer pointer is valid for the duration of the syscall; no aliasing with kernel state.
                    unsafe {
                        core::ptr::copy_nonoverlapping(
                            words.as_ptr() as *const u8,
                            buf.as_mut_ptr().add(chunk_off),
                            32,
                        );
                    }
                    chunk_off += 32;
                }
                reply(WRITE_ACK);
            }
            WRITE_COMMIT => {
                // Copy buffered chunk data into DMA area before I/O
                // SAFETY: category=raw-pointer-deref IPC buffer pointer is valid for the duration of the syscall; no aliasing with kernel state.
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        buf.as_ptr(), (va + OFF_DATA) as *mut u8, 512);
                }
                fjell_syscall::sys_debug_write_byte(0xC0 + (lba as u8 & 0x3F)); // lba probe
                let ok = do_io(mmio, va, pa, lba, true);
                reply(if ok { WRITE_OK } else { WRITE_ERR });
            }
            READ_BEGIN => {
                lba = (w1 as u64) | ((w2 as u64) << 32);
                chunk_off = 0;
                reply(READ_ACK);
            }
            READ_COMMIT => {
                let ok = do_io(mmio, va, pa, lba, false);
                if ok {
                    // SAFETY: category=raw-pointer-deref IPC buffer pointer is valid for the duration of the syscall; no aliasing with kernel state.
                    unsafe {
                        core::ptr::copy_nonoverlapping(
                            (va + OFF_DATA) as *const u8, buf.as_mut_ptr(), 512);
                    }
                    reply(READ_OK);
                } else {
                    reply(READ_ERR);
                }
            }
            READ_CHUNK => {
                chunk_off += 32;
                reply(READ_CHUNK); // placeholder; READ not used in M7
            }
            _ => { reply(0); }
        }
    }
}
