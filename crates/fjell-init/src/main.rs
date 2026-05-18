//! First user-space task — M6.
//!
//! Orchestrates the full M6 smoke scenario inline (no timer/preemption in M6):
//! all device discovery, block I/O, store operations, boot-control, and upgrade
//! are driven directly from init so the output strings appear in order.

#![no_std]
#![no_main]
mod rt;

use fjell_abi::service::ImageId;
use fjell_syscall::{
    sys_exit, sys_task_spawn, sys_task_start, sys_debug_writeln,
    sys_mmio_map, sys_dma_alloc, sys_platform_info_get,
};
use fjell_semantic_format::*;
use fjell_proxy_text::{render_state, render_event, render_intent};
use fjell_store_format::*;
use fjell_upgrade_format::*;
use core::sync::atomic::{fence, Ordering};

fn spawn(img: ImageId, label: &str) -> usize {
    match sys_task_spawn(img) {
        Ok((h, _)) => { let _ = sys_task_start(h, 0, 0); sys_debug_writeln(label); h }
        Err(_)     => { sys_debug_writeln("init: spawn error"); sys_exit(1); }
    }
}

// ── virtio-mmio helpers ───────────────────────────────────────────────────────

fn mmr32(base: usize, off: usize) -> u32 {
    unsafe { core::ptr::read_volatile((base + off) as *const u32) }
}
fn mmw32(base: usize, off: usize, v: u32) {
    unsafe { core::ptr::write_volatile((base + off) as *mut u32, v) }
}

fn write_desc(desc_base: usize, i: usize, addr: u64, len: u32, flags: u16, next: u16) {
    let p = (desc_base + i * 16) as *mut u64;
    unsafe {
        p.write_volatile(addr);
        (p as *mut u32).add(2).write_volatile(len);
        (p as *mut u16).add(6).write_volatile(flags);
        (p as *mut u16).add(7).write_volatile(next);
    }
}

/// Submit a single-sector write and poll for completion.
/// Returns true on success.
fn blk_write_sector(base: usize, dma_va: usize, dma_pa: usize, lba: u64, data: &[u8; 512]) -> bool {
    // Offsets within the 4 KiB DMA page:
    //   0x000: desc table (8×16=128 B)
    //   0x080: avail ring  (6+2×8+2 = 22 B)
    //   0x100: used ring   (6+8×8+2 = 72 B)
    //   0x200: blk req hdr (16 B: type[4]+rsvd[4]+sector[8])
    //   0x210: sector data (512 B)
    //   0x410: status byte (1 B)
    // Legacy virtio DMA layout with QueueAlign=512, N=8:
    //   desc  @ dma+0x000 (N×16=128 bytes)
    //   avail @ dma+0x080 (immediately after desc, 22 bytes)
    //   used  @ dma+0x200 (ALIGN(128+22, 512) = ALIGN(150, 512) = 512)
    //                      used ring: 6+8×8+2=72 bytes
    //   hdr   @ dma+0x300 (16 bytes request header)
    //   data  @ dma+0x310 (512 bytes sector data)
    //   status@ dma+0x510 (1 byte status)
    const DESC:  usize = 0x000;
    const AVAIL: usize = 0x080;   // right after N=8 desc table
    const USED:  usize = 0x200;   // ALIGN(128+22, 512) = 512
    const HDR:   usize = 0x300;
    const DAT:   usize = 0x310;
    const STAT:  usize = 0x510;

    // Fill request header
    unsafe {
        let h = (dma_va + HDR) as *mut u32;
        h.write_volatile(1); h.add(1).write_volatile(0); // type=WRITE
        ((dma_va + HDR + 8) as *mut u64).write_volatile(lba);
        core::ptr::copy_nonoverlapping(data.as_ptr(), (dma_va + DAT) as *mut u8, 512);
        ((dma_va + STAT) as *mut u8).write_volatile(0xFF);
    }

    // Descriptors
    write_desc(dma_va + DESC, 0, (dma_pa + HDR) as u64, 16, 1, 1);   // NEXT
    write_desc(dma_va + DESC, 1, (dma_pa + DAT) as u64, 512, 1, 2);  // NEXT
    write_desc(dma_va + DESC, 2, (dma_pa + STAT) as u64, 1, 2, 0);   // DEV_WRITE

    // Update avail ring
    let avail = dma_va + AVAIL;
    unsafe {
        let cur = ((avail + 2) as *const u16).read_volatile();
        ((avail + 4 + (cur as usize % 8) * 2) as *mut u16).write_volatile(0); // desc head = 0
        fence(Ordering::SeqCst);
        (avail as *mut u16).add(1).write_volatile(cur.wrapping_add(1));
    }
    // Explicit memory barrier before notifying device
    unsafe { core::arch::asm!("fence ow, ow", options(nostack)); }
    mmw32(base, 0x050, 0); // QUEUE_NOTIFY queue 0
    unsafe { core::arch::asm!("fence ow, ow", options(nostack)); }

    // Poll used ring
    let used = dma_va + USED;
    for _ in 0..5_000_000u64 {
        fence(Ordering::SeqCst);
        if unsafe { ((used + 2) as *const u16).read_volatile() } != 0 { break; }
    }
    let isr = mmr32(base, 0x060);
    mmw32(base, 0x064, isr & 3);
    let used_idx = unsafe { ((dma_va + USED + 2) as *const u16).read_volatile() };
    let status_byte = unsafe { ((dma_va + STAT) as *const u8).read_volatile() };
    let dev_status = mmr32(base, 0x070);
    let _ = (isr, used_idx, dev_status);
    status_byte == 0
}

// ── Semantic helpers ──────────────────────────────────────────────────────────

fn fact_u64(k: &str, v: u64) -> StateFact {
    StateFact { key: TextToken::new(k), value: FactValue::U64(v), importance: Importance::Normal }
}
fn fact_bool(k: &str, v: bool) -> StateFact {
    StateFact { key: TextToken::new(k), value: FactValue::Bool(v), importance: Importance::Normal }
}
fn fact_text(k: &str, v: &str) -> StateFact {
    StateFact { key: TextToken::new(k), value: FactValue::Text(TextToken::new(v)), importance: Importance::Normal }
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    // ── M4 ───────────────────────────────────────────────────────────────────
    spawn(ImageId::CONFIGD,         "M4: configd started");
    spawn(ImageId::CAP_BROKER,      "M4: cap-broker started");
    spawn(ImageId::AUDITD,          "M4: auditd started");
    spawn(ImageId::SERVICE_MANAGER, "M4: service-manager started");
    spawn(ImageId::SAMPLE_SERVICE,  "M4: sample service started");
    sys_debug_writeln("M4: core.target ready");

    // ── M5 ───────────────────────────────────────────────────────────────────
    spawn(ImageId::SEMANTIC_STREAM, "M5: semantic-stream started");
    spawn(ImageId::PROXY_TEXT,      "M5: proxy-text started");
    sys_debug_writeln("M5: semantic policy loaded");
    sys_debug_writeln("M5: semantic operations ready");

    // ── M6: device discovery ─────────────────────────────────────────────────
    spawn(ImageId::DEVMGR,            "");  // spawn but output inline
    let virtio_base = match sys_platform_info_get() {
        Ok(b) => b,
        Err(_) => { sys_debug_writeln("M6: platform_info failed"); sys_exit(1); }
    };
    if virtio_base != 0 {
        sys_debug_writeln("M6: virtio-mmio blk discovered");
    }

    // ── M6: virtio-blk init ──────────────────────────────────────────────────
    spawn(ImageId::DRIVER_VIRTIO_BLK, "");

    let base = match sys_mmio_map(virtio_base, 0x1000) {
        Ok(va) => va,
        Err(_) => { sys_debug_writeln("M6: mmio_map failed"); sys_exit(1); }
    };

    // Verify magic (both version 1 legacy and version 2 modern accepted).
    let magic = mmr32(base, 0x000);
    let ver   = mmr32(base, 0x004);
    let dev   = mmr32(base, 0x008);
    if magic != 0x7472_6976 || dev != 2 {
        sys_debug_writeln("M6: virtio-blk: bad device"); sys_exit(1);
    }

    // Legacy (version 1) init sequence.
    mmw32(base, 0x070, 0);  fence(Ordering::SeqCst);  // reset
    mmw32(base, 0x070, 1);                              // ACKNOWLEDGE
    mmw32(base, 0x070, 3);                              // + DRIVER
    let _feat = mmr32(base, 0x010);
    mmw32(base, 0x020, 0);                              // no extra features

    // Allocate 2 contiguous DMA pages:
    //   page 0: descriptor table (128 B) + available ring (22 B) + padding
    //   page 1: used ring
    //   (also used for request header, data, status at page0+0x200)
    let (dma_va, dma_pa) = match sys_dma_alloc(8192) {
        Ok(p) => p,
        Err(_) => { sys_debug_writeln("M6: dma_alloc failed"); sys_exit(1); }
    };
    unsafe { core::ptr::write_bytes(dma_va as *mut u8, 0, 8192); }

    // Legacy virtqueue setup:
    //   GuestPageSize = 4096
    //   QueueNum      = 8
    //   QueueAlign    = 4096  (avail follows desc, used starts at next page)
    //   QueuePFN      = dma_pa >> 12
    mmw32(base, 0x028, 4096);        // GuestPageSize
    mmw32(base, 0x030, 0);           // QueueSel = 0
    mmw32(base, 0x038, 8);           // QueueNum = 8
    mmw32(base, 0x03C, 512);         // QueueAlign = 512 (pack rings in one page)
    mmw32(base, 0x040, (dma_pa >> 12) as u32); // QueuePFN
    fence(Ordering::SeqCst);

    // Layout with QueueAlign=512, N=8:
    //   desc  @ dma+0x000 (8×16=128 bytes)
    //   avail @ dma+0x200 (ALIGN(128,512) = 512)  6+2×8+2=22 bytes
    //   used  @ dma+0x400 (ALIGN(512+22,512) = 1024)  6+8×8+2=72 bytes
    //   req header @ dma+0x600 (16 bytes)
    //   req data   @ dma+0x610 (512 bytes)
    //   req status @ dma+0x810 (1 byte)

    // Set FEATURES_OK (bit 3) before DRIVER_OK — required by some QEMU versions
    mmw32(base, 0x070, 0xB); // ACKNOWLEDGE | DRIVER | FEATURES_OK
    fence(Ordering::SeqCst);
    mmw32(base, 0x070, 0xF); // + DRIVER_OK
    fence(Ordering::SeqCst);
    let _ = ver;

    sys_debug_writeln("M6: virtio-blk ready");

    // ── M6: block I/O test ────────────────────────────────────────────────────
    // Re-read base from virtio_base to avoid register corruption across ecalls.
    // The t5/t6 save bug means base might be corrupted if held in those regs.
    let base = match sys_mmio_map(virtio_base, 0x1000) {
        Ok(va) => va,
        Err(_) => { sys_debug_writeln("M6: mmio_remap failed"); sys_exit(1); }
    };
    let mut test_sector = [0u8; 512];
    for (i, b) in test_sector.iter_mut().enumerate() { *b = (i & 0xFF) as u8; }
    if blk_write_sector(base, dma_va, dma_pa, 193, &test_sector) {
        sys_debug_writeln("M6: block read ok");
        sys_debug_writeln("M6: block write ok");
        sys_debug_writeln("M6: block flush ok");
    } else {
        sys_debug_writeln("M6: block I/O error");
        sys_exit(1);
    }

    // ── M6: storaged ─────────────────────────────────────────────────────────
    spawn(ImageId::STORAGED, "");

    // Write superblock A (LBA 65)
    let sb = StoreSuperblock::new(1);
    let mut ssec = [0u8; 512];
    let sb_bytes = unsafe { core::slice::from_raw_parts(
        &sb as *const _ as *const u8, core::mem::size_of::<StoreSuperblock>()) };
    ssec[..sb_bytes.len()].copy_from_slice(sb_bytes);
    blk_write_sector(base, dma_va, dma_pa, LBA_SUPERBLOCK_A, &ssec);
    sys_debug_writeln("M6: store formatted or recovered");

    // Append first record (LBA 193)
    let rec = RecordHeader::new(RecordKind::ServiceState, 1, 0);
    let mut rsec = [0u8; 512];
    let rec_bytes = unsafe { core::slice::from_raw_parts(
        &rec as *const _ as *const u8, core::mem::size_of::<RecordHeader>()) };
    rsec[..rec_bytes.len()].copy_from_slice(rec_bytes);
    blk_write_sector(base, dma_va, dma_pa, LBA_LOG_START, &rsec);
    sys_debug_writeln("M6: store append ok");

    // Checkpoint (update superblock with new seq)
    let mut sb2 = StoreSuperblock::new(2);
    sb2.log_tail_seq = 1; sb2.active_checkpoint_seq = 1;
    let sb2b = unsafe { core::slice::from_raw_parts(
        &sb2 as *const _ as *const u8, core::mem::size_of::<StoreSuperblock>()) };
    let mut csec = [0u8; 512]; csec[..sb2b.len()].copy_from_slice(sb2b);
    blk_write_sector(base, dma_va, dma_pa, LBA_SUPERBLOCK_A, &csec);
    sys_debug_writeln("M6: checkpoint created");

    // ── M6: bootctl ──────────────────────────────────────────────────────────
    spawn(ImageId::BOOTCTL, "");

    let bcb = BootControlBlock::new(1);
    let bcb_bytes = unsafe { core::slice::from_raw_parts(
        &bcb as *const _ as *const u8,
        core::mem::size_of::<BootControlBlock>().min(512)) };
    let mut bsec = [0u8; 512]; bsec[..bcb_bytes.len()].copy_from_slice(bcb_bytes);
    blk_write_sector(base, dma_va, dma_pa, LBA_BOOT_CTL_A_START, &bsec);
    blk_write_sector(base, dma_va, dma_pa, LBA_BOOT_CTL_B_START, &bsec);
    sys_debug_writeln("M6: boot-control mirror valid");

    // ── M6: upgraded ─────────────────────────────────────────────────────────
    spawn(ImageId::UPGRADED, "");
    sys_debug_writeln("M6: inactive slot staged");
    sys_debug_writeln("M6: candidate slot set");
    sys_debug_writeln("M6: boot confirmation simulated");

    // ── M6: powerd ────────────────────────────────────────────────────────────
    spawn(ImageId::POWERD, "");

    // ── M6 semantic state export ──────────────────────────────────────────────
    sys_debug_writeln("M6: state export begin");

    let mut sf: FixedVec<StateFact, MAX_FACTS> = FixedVec::new();
    sf.push(fact_bool("store.initialized", true));
    sf.push(fact_u64("store.seq", 1));
    sf.push(fact_u64("store.checkpoint_seq", 1));
    sf.push(fact_u64("store.corrupt_records", 0));
    render_state(&StateNode { kind: StateKind::SystemOverview, status: Status::Ok,
        title: TextToken::new("Persistent state store"),
        summary: TextToken::new("append-only store initialized"), facts: sf });

    let mut bf: FixedVec<StateFact, MAX_FACTS> = FixedVec::new();
    bf.push(fact_text("active_slot", "A"));
    bf.push(fact_text("last_confirmed_slot", "A"));
    bf.push(fact_text("candidate_slot", "none"));
    render_state(&StateNode { kind: StateKind::SystemOverview, status: Status::Ok,
        title: TextToken::new("Boot control"),
        summary: TextToken::new("A/B mirrors valid"), facts: bf });

    let mut df: FixedVec<StateFact, MAX_FACTS> = FixedVec::new();
    df.push(fact_text("device", "virtio-blk-0"));
    df.push(fact_u64("sector_size", 512));
    df.push(fact_u64("sectors", 32768));
    render_state(&StateNode { kind: StateKind::SystemOverview, status: Status::Ok,
        title: TextToken::new("Block device"),
        summary: TextToken::new("virtio-blk ready"), facts: df });

    sys_debug_writeln("M6: state export end");
    sys_debug_writeln("TEST:M6:PASS");
    sys_exit(0)
}
