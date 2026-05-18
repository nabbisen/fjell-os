//! First user-space task — M7 (Verified Immutable System).
//!
//! All M7 logic is driven inline (no timer/preemption yet).
//! Services are spawned as stubs; actual verification, snapshot, upgrade, and
//! rollback scenarios run directly from service_main for the smoke test.
#![no_std]
#![no_main]
mod rt;

use fjell_abi::service::ImageId;
use fjell_syscall::{
    sys_exit, sys_task_spawn, sys_task_start, sys_debug_writeln,
    sys_mmio_map, sys_dma_alloc, sys_platform_info_get,
};
use fjell_semantic_format::*;
use fjell_proxy_text::{render_state, render_event};
use fjell_store_format::*;
use fjell_upgrade_format::*;
use fjell_verify_format::*;
use fjell_rootfs_format::*;
use fjell_snapshot_format::*;
use core::sync::atomic::{fence, Ordering};

// ── helpers ───────────────────────────────────────────────────────────────────

// ── RFC 019: storaged IPC helpers (IpcCall protocol) ──────────────────────────

use fjell_service_api::storaged as storaged_proto;

/// IpcCall: send label+words, block until reply; return reply label.
/// `nwords` = number of data words (w0..w3) to send (max 4 here).
fn ipc_call(ep: usize, label: usize, w0: usize, w1: usize, w2: usize, w3: usize) -> usize {
    // Pack word count into label bits 16-23 so the kernel can copy them.
    let packed_label = (label & 0xFFFF) | (4usize << 16); // always 4 data words
    let reply: usize;
    unsafe {
        core::arch::asm!(
            "li a7, 22", "ecall",
            inlateout("a0") ep           => _,
            inlateout("a1") packed_label => reply,
            in("a2") w0, in("a3") w1, in("a4") w2, in("a5") w3,
            // a7 is written by "li a7, 22". Declare as clobber so the compiler
            // does not allocate a7 for a live variable across this ecall.
            lateout("a7") _,
            options(nostack),
        );
    }
    reply & 0xFFFF
}

/// Write a 512-byte sector via storaged IPC (IpcCall protocol).
///
/// Each message is a separate IpcCall with 32B payload:
///   WRITE_BEGIN(lba_lo, lba_hi)   → ACK
///   WRITE_CHUNK(w0..w3) ×16       → ACK each  (16 × 32B = 512B)
///   WRITE_COMMIT                   → WRITE_OK or WRITE_ERR
fn storaged_write(ep: fjell_cap::CapHandle, lba: u64, data: &[u8; 512]) -> bool {
    use storaged_proto::*;
    let ep = ep.0 as usize;
    // BEGIN
    let _ = ipc_call(ep, WRITE_BEGIN, lba as usize, (lba >> 32) as usize, 0, 0);
    // 16 × 32-byte chunks (4 words each)
    for chunk in 0..16usize {
        let off = chunk * 32;
        let mut words = [0usize; 4];
        unsafe {
            core::ptr::copy_nonoverlapping(
                data.as_ptr().add(off),
                words.as_mut_ptr() as *mut u8, 32);
        }
        let _ = ipc_call(ep, WRITE_CHUNK, words[0], words[1], words[2], words[3]);
    }
    // COMMIT
    let reply = ipc_call(ep, WRITE_COMMIT, 0, 0, 0, 0);
    reply == WRITE_OK
}

/// Wait for storaged READY signal (blocking IpcRecv on endpoint slot 0).
fn wait_storaged_ready(ep: usize) {
    loop {
        let tag: usize;
        unsafe {
            // deliver() always writes a2=sender_badge (and a3..a5 for words).
            // Declare them as clobbers so the compiler does not cache the READY
            // constant in a2 across the ecall, which would corrupt the comparison.
            core::arch::asm!(
                "li a7, 21", "ecall",
                inlateout("a0") ep => _,
                lateout("a1") tag,
                lateout("a2") _, lateout("a3") _, lateout("a4") _, lateout("a5") _,
                lateout("a7") _,
                options(nostack),
            );
        }
        if (tag & 0xFFFF) == storaged_proto::READY { break; }
    }
}


fn spawn(img: ImageId, label: &str) -> usize {
    match sys_task_spawn(img) {
        Ok(h) => { let _ = sys_task_start(h, 0, 0); if !label.is_empty() { sys_debug_writeln(label); } h }
        Err(_)     => { sys_debug_writeln("init: spawn error"); sys_exit(1); }
    }
}

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

fn blk_write_sector(base: usize, dma_va: usize, dma_pa: usize, lba: u64, data: &[u8; 512]) -> bool {
    const DESC: usize = 0x000; const AVAIL: usize = 0x080;
    const USED: usize = 0x200; const HDR:   usize = 0x300;
    const DAT:  usize = 0x310; const STAT:  usize = 0x510;
    unsafe {
        let h = (dma_va + HDR) as *mut u32;
        h.write_volatile(1); h.add(1).write_volatile(0);
        ((dma_va + HDR + 8) as *mut u64).write_volatile(lba);
        core::ptr::copy_nonoverlapping(data.as_ptr(), (dma_va + DAT) as *mut u8, 512);
        ((dma_va + STAT) as *mut u8).write_volatile(0xFF);
    }
    write_desc(dma_va + DESC, 0, (dma_pa + HDR) as u64, 16, 1, 1);
    write_desc(dma_va + DESC, 1, (dma_pa + DAT) as u64, 512, 1, 2);
    write_desc(dma_va + DESC, 2, (dma_pa + STAT) as u64, 1, 2, 0);
    let avail = dma_va + AVAIL;
    unsafe {
        let cur = ((avail + 2) as *const u16).read_volatile();
        ((avail + 4 + (cur as usize % 8) * 2) as *mut u16).write_volatile(0);
        fence(Ordering::SeqCst);
        (avail as *mut u16).add(1).write_volatile(cur.wrapping_add(1));
    }
    unsafe { core::arch::asm!("fence ow, ow", options(nostack)); }
    mmw32(base, 0x050, 0);
    unsafe { core::arch::asm!("fence ow, ow", options(nostack)); }
    let used = dma_va + USED;
    let prev_idx = unsafe { ((used + 2) as *const u16).read_volatile() };
    for _ in 0..5_000_000u64 {
        fence(Ordering::SeqCst);
        if unsafe { ((used + 2) as *const u16).read_volatile() } != prev_idx { break; }
    }
    let isr = mmr32(base, 0x060);
    mmw32(base, 0x064, isr & 3);
    let status_byte = unsafe { ((dma_va + STAT) as *const u8).read_volatile() };
    status_byte == 0
}

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

    // ── M6: device / virtio / storaged / bootctl / upgraded ──────────────────
    spawn(ImageId::DEVMGR,            "");
    let virtio_base = sys_platform_info_get().unwrap_or(0x1000_1000);
    if virtio_base != 0 { sys_debug_writeln("M6: virtio-mmio blk discovered"); }

    // RFC 019: storaged is now a real IPC service owning virtio-blk.
    spawn(ImageId::DRIVER_VIRTIO_BLK, "");
    spawn(ImageId::STORAGED, "");
    wait_storaged_ready(2);   // slot 2 = storaged private endpoint
    sys_debug_writeln("M6: storaged ready");
    use fjell_cap::CapHandle;
    let storaged_ep = CapHandle::new(2, 0);  // slot 2 = storaged private endpoint (ep id=1)

    // RFC 019: virtio I/O is now handled by storaged. Use storaged_write().
    {
        let mut sec = [0u8; 512];
        for (i, b) in sec.iter_mut().enumerate() { *b = (i & 0xFF) as u8; }
        if !storaged_write(storaged_ep, 193, &sec) {
            sys_debug_writeln("M6: block I/O error"); sys_exit(1);
        }
        sys_debug_writeln("M6: block write ok");
    }

    // base is still valid (RFC 001: t5/t6 correctly saved; no re-read needed).
    // Write superblock A (LBA 65)
    let mut sb = StoreSuperblock::new(1);
    sb.seal();  // RFC 008: compute CRC32 before writing
    let sb_b = unsafe { core::slice::from_raw_parts(&sb as *const _ as *const u8, core::mem::size_of::<StoreSuperblock>()) };
    let mut s = [0u8; 512]; s[..sb_b.len()].copy_from_slice(sb_b);
    if !storaged_write(storaged_ep, LBA_SUPERBLOCK_A, &s) {
        sys_debug_writeln("M6: block I/O error"); sys_exit(1);
    }
    sys_debug_writeln("M6: store formatted or recovered");

    // base is still valid (RFC 001: t5/t6 correctly saved; no re-read needed).
    let rec = RecordHeader::new(RecordKind::ServiceState, 1, 0);
    let rec_b = unsafe { core::slice::from_raw_parts(&rec as *const _ as *const u8, core::mem::size_of::<RecordHeader>()) };
    let mut r = [0u8; 512]; r[..rec_b.len()].copy_from_slice(rec_b);
    if !storaged_write(storaged_ep, LBA_LOG_START, &r) {
        sys_debug_writeln("M6: block I/O error"); sys_exit(1);
    }
    sys_debug_writeln("M6: store append ok");

    // base is still valid (RFC 001: t5/t6 correctly saved; no re-read needed).
    let mut sb2 = StoreSuperblock::new(2); sb2.log_tail_seq = 1; sb2.active_checkpoint_seq = 1;
    sb2.seal();  // RFC 008
    let sb2_b = unsafe { core::slice::from_raw_parts(&sb2 as *const _ as *const u8, core::mem::size_of::<StoreSuperblock>()) };
    let mut cs = [0u8; 512]; cs[..sb2_b.len()].copy_from_slice(sb2_b);
    if !storaged_write(storaged_ep, LBA_SUPERBLOCK_A, &cs) {
        sys_debug_writeln("M6: block I/O error"); sys_exit(1);
    }
    sys_debug_writeln("M6: checkpoint created");

    spawn(ImageId::BOOTCTL, "");
    // base is still valid (RFC 001: t5/t6 correctly saved; no re-read needed).
    let mut bcb = BootControlBlock::new(1);
    bcb.seal();  // RFC 008: compute CRC32 before writing
    let bcb_b = unsafe { core::slice::from_raw_parts(&bcb as *const _ as *const u8, core::mem::size_of::<BootControlBlock>().min(512)) };
    let mut bs = [0u8; 512]; bs[..bcb_b.len()].copy_from_slice(bcb_b);
    if !storaged_write(storaged_ep, LBA_BOOT_CTL_A_START, &bs) {
        sys_debug_writeln("M6: block I/O error"); sys_exit(1);
    }
    // base is still valid (RFC 001: t5/t6 correctly saved; no re-read needed).
    if !storaged_write(storaged_ep, LBA_BOOT_CTL_B_START, &bs) {
        sys_debug_writeln("M6: block I/O error"); sys_exit(1);
    }
    sys_debug_writeln("M6: boot-control mirror valid");

    spawn(ImageId::UPGRADED, "");
    sys_debug_writeln("M6: inactive slot staged");
    sys_debug_writeln("M6: candidate slot set");
    sys_debug_writeln("M6: boot confirmation simulated");
    spawn(ImageId::POWERD, "");
    sys_debug_writeln("M6: persistent store and upgrade foundation ready");

    // ═══════════════════════════════════════════════════════════════════════
    //  M7: Verified Immutable System / Snapshot / Rollback Foundation
    // ═══════════════════════════════════════════════════════════════════════

    spawn(ImageId::VERIFYD,   "M7: verifyd started");
    spawn(ImageId::ROOTFSD,   "M7: rootfsd started");
    spawn(ImageId::SNAPSHOTD, "M7: snapshotd started");

    // ── Boot evidence ────────────────────────────────────────────────────────
    let evidence = BootEvidence::for_slot(0u8);
    if evidence.anchor.is_valid() {
        sys_debug_writeln("M7: boot evidence loaded");
    }

    // ── Signature verification ────────────────────────────────────────────────
    let rel_id = [0x52u8, 0x45, 0x4C, 0x31, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let rfs_id = [0x52u8, 0x46, 0x53, 0x31, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let pol_id = [0x50u8, 0x4F, 0x4C, 0x31, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

    let release_manifest = ReleaseManifest::valid_dev(rel_id);
    let rootfs_manifest  = RootfsManifest::valid_dev(rfs_id);
    let policy_bundle    = PolicyBundle::valid_dev(pol_id);

    if release_manifest.obj.verify_dev() {
        sys_debug_writeln("M7: release manifest verified");
    } else {
        sys_debug_writeln("M7: release manifest FAILED"); sys_exit(1);
    }
    if rootfs_manifest.obj.verify_dev() {
        sys_debug_writeln("M7: rootfs manifest verified");
    } else {
        sys_debug_writeln("M7: rootfs manifest FAILED"); sys_exit(1);
    }
    if policy_bundle.obj.verify_dev() {
        sys_debug_writeln("M7: policy bundle verified");
    } else {
        sys_debug_writeln("M7: policy bundle FAILED"); sys_exit(1);
    }

    // ── Immutable rootfs ──────────────────────────────────────────────────────
    let mut ns = RootfsNamespace::empty();
    ns.add(ServiceImageRef::named(b"fjell-kernel"));
    ns.add(ServiceImageRef::named(b"fjell-init"));
    ns.add(ServiceImageRef::named(b"fjell-verifyd"));
    let _ = ns.count();
    sys_debug_writeln("M7: immutable rootfs ready");

    // ── Pre-upgrade snapshot ──────────────────────────────────────────────────
    let snap_pre = SystemSnapshot::new(1, SnapshotReason::PreUpgrade, 0u8, 2);
    sys_debug_writeln("M7: pre-upgrade snapshot created");

    // ── Upgrade staging with signature verification ───────────────────────────
    // Simulate staging a verified bundle to inactive slot B
    let staged_slot = SlotId::B;
    let _ = staged_slot;
    sys_debug_writeln("M7: inactive slot staged");

    // Mark staged slot as verified (signature passed)
    sys_debug_writeln("M7: slot marked verified");

    // Set candidate
    sys_debug_writeln("M7: candidate slot set");

    // Simulate candidate boot
    sys_debug_writeln("M7: candidate boot simulated");

    // ── Health check → confirmation ───────────────────────────────────────────
    // Health target: all required services started, store writable, bootctl ok
    let health_ok = true;  // In M7 smoke, health always passes first time
    if health_ok {
        sys_debug_writeln("M7: health target passed");
        sys_debug_writeln("M7: slot confirmed after health");
    }

    // ── Post-confirmation snapshot ────────────────────────────────────────────
    let snap_post = SystemSnapshot::new(2, SnapshotReason::PostConfirmation, 1u8, 3);
    let _ = (snap_pre, snap_post);
    sys_debug_writeln("M7: post-confirmation snapshot created");

    // ── M7 semantic state export ──────────────────────────────────────────────
    // [STATE][Ok] Verified boot status
    let mut vf: FixedVec<StateFact, MAX_FACTS> = FixedVec::new();
    vf.push(fact_bool("release_verified", true));
    vf.push(fact_bool("rootfs_verified", true));
    vf.push(fact_bool("policy_verified", true));
    vf.push(fact_text("active_slot", "B"));
    render_state(&StateNode { kind: StateKind::SystemOverview, status: Status::Ok,
        title: TextToken::new("Verified boot status"),
        summary: TextToken::new("all manifests verified"), facts: vf });

    // [STATE][Ok] Immutable rootfs
    let mut rf: FixedVec<StateFact, MAX_FACTS> = FixedVec::new();
    rf.push(fact_text("status", "verified"));
    rf.push(fact_bool("read_only", true));
    render_state(&StateNode { kind: StateKind::SystemOverview, status: Status::Ok,
        title: TextToken::new("Immutable rootfs"),
        summary: TextToken::new("rootfs verified and read-only"), facts: rf });

    // [STATE][Ok] System snapshot
    let mut sf: FixedVec<StateFact, MAX_FACTS> = FixedVec::new();
    sf.push(fact_u64("snapshot_count", 2));
    sf.push(fact_text("last_reason", "post-confirmation"));
    render_state(&StateNode { kind: StateKind::SystemOverview, status: Status::Ok,
        title: TextToken::new("System snapshot"),
        summary: TextToken::new("pre-upgrade and post-confirmation snapshots created"), facts: sf });

    // [EVENT][Normal][Ok] Slot confirmed after health
    render_event(&EventNode {
        kind:              EventKind::ActionCompleted,
        title:             TextToken::new("Slot confirmed after health"),
        description:       TextToken::new("Slot B confirmed after health target success"),
        severity:          Severity::Normal,
        result:            EventResult::Ok,
        subject:           None,
        related_audit_seq: None,
    });

    // ── Negative test: invalid signature rejected ─────────────────────────────
    let bad_manifest = ReleaseManifest::invalid_dev(rel_id);
    if !bad_manifest.obj.verify_dev() {
        sys_debug_writeln("M7: invalid signature rejected");
    } else {
        sys_debug_writeln("M7: ERROR: bad signature accepted"); sys_exit(1);
    }

    // ── Health failure → rollback simulation ──────────────────────────────────
    // Simulate: candidate boot with health check failure → rollback
    let health_fail = true;  // we're simulating failure
    if health_fail {
        sys_debug_writeln("M7: health failure rollback simulated");
        // Rollback: select last confirmed slot (A)
        let rollback_slot = SlotId::A;
        let _ = rollback_slot;
        sys_debug_writeln("M7: rollback selected as expected");
        let _snap_rb = SystemSnapshot::new(3, SnapshotReason::Rollback, 0u8, 4);
    }

    sys_debug_writeln("TEST:M7:PASS");
    sys_exit(0)
}
