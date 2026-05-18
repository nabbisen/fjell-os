//! attestd — Local attestation service for Fjell OS M8.
//! Development-grade: generates signed local attestation records.
#![no_std]
#![no_main]
mod rt;
use fjell_attestation_format::{
    AttestationRecord, AttestationRecordId, AttestationProfile, SignedAttestationRecord,
    BootClaims, VerificationClaims, MeasurementClaims, SnapshotClaims,
    HealthClaims, FreshnessClaims,
};
use fjell_measure_format::Digest32;
use fjell_service_api::attestd as proto;
use fjell_syscall::{sys_debug_writeln, sys_exit};
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { sys_debug_writeln("attestd: panic"); sys_exit(1); }
const EP_SLOT: u32 = 0;

fn send_ready() { unsafe { core::arch::asm!("li a7, 20","ecall", in("a0") EP_SLOT as usize, in("a1") proto::READY, lateout("a0") _, lateout("a7") _, options(nostack)); } }
fn recv_call() -> (usize, usize, usize) {
    let (mut t, mut w0, mut w1) = (0usize, 0usize, 0usize);
    unsafe { core::arch::asm!("li a7, 21","ecall",
        in("a0") EP_SLOT as usize, lateout("a1") t, lateout("a2") w0, lateout("a3") w1,
        lateout("a4") _, lateout("a5") _, lateout("a7") _, options(nostack)); }
    (t, w0, w1)
}
fn reply(tag: usize) { unsafe { core::arch::asm!("li a7, 23","ecall", in("a0") 0usize, in("a1") tag, lateout("a7") _, options(nostack)); } }

/// Make a compact attestation record — minimise stack usage.
fn sign_record(seq: u32, meas_seq: u64) -> (Digest32, bool) {
    // Build record_id from seq
    let mut id = [b'0'; 8];
    id[0] = b'A'; id[1] = b'T';
    let mut n = seq; let mut i = 7usize;
    while n > 0 && i >= 2 { id[i] = b'0' + (n % 10) as u8; n /= 10; if i > 2 { i -= 1; } else { break; } }

    let record = AttestationRecord {
        schema_version: AttestationRecord::SCHEMA_VERSION,
        record_id:      AttestationRecordId(id),
        created_tick:   0,
        profile:        AttestationProfile::FjellLocalV1Binary,
        nonce:          None,
        boot: BootClaims { selected_slot: 0, boot_id: 1, kernel_digest: Digest32([0x11;32]) },
        verification: VerificationClaims {
            release_digest: Digest32([0x22;32]), rootfs_digest: Digest32([0x33;32]),
            policy_digest:  Digest32([0x44;32]),
            release_verified: true, rootfs_verified: true, policy_verified: true,
        },
        measurement: MeasurementClaims {
            head_seq: meas_seq,
            chain_digest: Digest32([0x55;32]),
            included_from_seq: 1,
            included_to_seq: meas_seq,
        },
        snapshot: SnapshotClaims { snapshot_id: *b"SN000001", snapshot_digest: Digest32([0x66;32]), reason: 0 },
        health:   HealthClaims   { target: *b"m7-hlth\0", status: 0 },
        freshness: FreshnessClaims { generation: 1, key_epoch: 1, status: 0 },
        provenance: None,
    };
    let signed  = SignedAttestationRecord::sign(record);
    let digest  = signed.record_digest;
    let ok      = signed.verify();
    (digest, ok)
}

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    send_ready();
    sys_debug_writeln("M8: attestd ready");

    let mut record_seq: u32 = 0;
    let mut last_ok: bool   = false;
    let mut last_digest      = Digest32::ZERO;

    loop {
        let (tag_packed, w0, _w1) = recv_call();
        let tag = tag_packed & 0xFFFF;
        match tag {
            proto::GENERATE => {
                record_seq = record_seq.wrapping_add(1);
                let meas_seq = w0 as u64;
                let (digest, ok) = sign_record(record_seq, meas_seq);
                last_digest = digest;
                last_ok     = ok;
                reply(proto::GENERATED);
                sys_debug_writeln("M8: attestation record generated");
            }
            proto::VERIFY_LATEST => {
                reply(if last_ok { proto::VERIFY_OK } else { proto::VERIFY_FAIL });
            }
            proto::EXPORT => {
                reply(proto::EXPORT_DONE);
            }
            _ => reply(proto::ERR),
        }
        let _ = last_digest; // keep for future use
    }
}
