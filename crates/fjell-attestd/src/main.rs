//! attestd — Local attestation service for Fjell OS.
//!
//! v0.3.0: produces `AttestationRecordV2` when the trust-provider registry
//! is in Enforcing phase (RFC v0.3-004).  Falls back to the v1 dev path
//! when still in Bootstrap (development / early boot).
#![no_std]
#![no_main]
mod rt;

use fjell_attestation_format::{
    AttestationRecord, AttestationRecordId, AttestationProfile, SignedAttestationRecord,
    BootClaims, VerificationClaims, MeasurementClaims, SnapshotClaims,
    HealthClaims, FreshnessClaims,
};
use fjell_attestation_format::v2::{
    AttestationRecordV2, FreshnessClaimsV2, KeyringClaims, NonceClass,
    ProviderClaims, RollbackClaims, SignedAttestationRecordV2, SignedByDescriptor,
};
use fjell_measure_format::Digest32;
use fjell_service_api::attestd as proto;
use fjell_syscall::{sys_debug_writeln, sys_exit};

use fjell_trust_provider::descriptor::TrustProviderDescriptor;
use fjell_trust_provider::development::DevelopmentTrustProvider;
use fjell_trust_provider::ids::TrustProviderId;
#[allow(unused_imports)] // v0.7: hardware attestation material
use fjell_trust_provider::material::AttestationDigest;
use fjell_trust_provider::profile::{
    TrustProviderCapabilities, TrustProviderKind, TrustProfile, TrustProviderState,
};
#[allow(unused_imports)] // v0.7: hardware trust provider integration
use fjell_trust_provider::provider::HardwareTrustProvider;
use fjell_trust_provider::registry::{ProviderRegistry, RegistryPhase};

use fjell_keyring::algorithm::SignatureAlgorithm;
use fjell_keyring::anchor::{AuthorityClass, TrustAnchor};
use fjell_keyring::epoch::KeyEpoch;
use fjell_keyring::KeyPurpose;
use fjell_keyring::keyring::Keyring;

#[allow(unused_imports)] // v0.7: AdvanceSource used in full rollback tracking
use fjell_upgrade_format::rollback_record::{AdvanceSource, RollbackRecord};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    sys_debug_writeln("attestd: panic");
    sys_exit(1);
}

// ── IPC helpers ─────────────────────────────────────────────────────────────

const EP_SLOT: u32 = 0;

fn send_ready() {
    // SAFETY: shared-memory region is capability-gated; pointer is valid for the agreed-upon length.
    unsafe {
        core::arch::asm!(
            "li a7, 20", "ecall",
            in("a0") EP_SLOT as usize, in("a1") proto::READY,
            lateout("a0") _, lateout("a7") _, options(nostack)
        );
    }
}

fn recv_call() -> (usize, usize, usize) {
    let (mut t, mut w0, mut w1) = (0usize, 0usize, 0usize);
    // SAFETY: shared-memory region is capability-gated; pointer is valid for the agreed-upon length.
    unsafe {
        core::arch::asm!(
            "li a7, 21", "ecall",
            in("a0") EP_SLOT as usize,
            lateout("a1") t, lateout("a2") w0, lateout("a3") w1,
            lateout("a4") _, lateout("a5") _, lateout("a7") _, options(nostack)
        );
    }
    (t, w0, w1)
}

fn reply(tag: usize) {
    // SAFETY: shared-memory region is capability-gated; pointer is valid for the agreed-upon length.
    unsafe {
        core::arch::asm!(
            "li a7, 23", "ecall",
            in("a0") 0usize, in("a1") tag,
            lateout("a7") _, options(nostack)
        );
    }
}

// ── Trust-provider / keyring init ────────────────────────────────────────────

const ATTESTD_PROVIDER_ID: TrustProviderId = TrustProviderId::new(0x01);
const DEV_ANCHOR_KEY: [u8; 32] = [0u8; 32];
const DEV_CHANNEL: [u8; 8] = *b"dev\0\0\0\0\0";

fn dev_descriptor() -> TrustProviderDescriptor {
    TrustProviderDescriptor::new(
        ATTESTD_PROVIDER_ID,
        TrustProviderKind::Development,
        TrustProfile::FjellLocalV1,
        TrustProviderCapabilities::DEVELOPMENT_BASELINE,
        TrustProviderState::Active,
        1,
        *b"attestd\0",
    )
}

fn init_keyring() -> Keyring {
    let mut keyring = Keyring::new();
    if let Some(anchor) = TrustAnchor::new(
        KeyPurpose::AttestationSigning,
        SignatureAlgorithm::DevDigest32,
        AuthorityClass::Genesis,
        KeyEpoch::ONE,
        &DEV_ANCHOR_KEY,
    ) {
        let _ = keyring.install(anchor);
    }
    keyring
}

// ── Record generation ─────────────────────────────────────────────────────────

/// Build the common v1-era record id from `seq`.
fn make_record_id(seq: u32) -> AttestationRecordId {
    let mut id = [b'0'; 8];
    id[0] = b'A'; id[1] = b'T';
    let mut n = seq; let mut i = 7usize;
    while n > 0 && i >= 2 {
        id[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if i > 2 { i -= 1; } else { break; }
    }
    AttestationRecordId(id)
}

/// Sign a v2 record when the registry is in Enforcing phase.
fn sign_v2(
    provider:   &DevelopmentTrustProvider,
    keyring:    &Keyring,
    rollback:   &RollbackRecord,
    seq:        u32,
    #[allow(dead_code)] // v0.7: forwarded to measurement chain header
    meas_seq:   u64,
    generation: u32,
) -> (Digest32, bool) {
    let record = AttestationRecordV2::dev(
        make_record_id(seq),
        ATTESTD_PROVIDER_ID,
        DEV_CHANNEL,
        [0u8; 16], // dev nonce
        rollback.min_counter,
        generation,
    );

    // Build signed_by from the keyring.
    let epoch = keyring
        .anchors_for(KeyPurpose::AttestationSigning)
        .next()
        .map(|a| a.epoch.raw())
        .unwrap_or(1);

    let signed_by = SignedByDescriptor {
        provider_id:          ATTESTD_PROVIDER_ID,
        provider_generation:  1,
        keyring_anchor_epoch: epoch,
        algorithm:            SignatureAlgorithm::DevDigest32 as u8,
    };

    match SignedAttestationRecordV2::sign(record, provider, signed_by) {
        Ok(s) => (s.record_digest, s.verify(provider)),
        Err(_) => (Digest32::ZERO, false),
    }
}

/// Sign a v1 record (fallback path — Bootstrap phase or no Enforcing anchor).
fn sign_v1(seq: u32, meas_seq: u64) -> (Digest32, bool) {
    let record = AttestationRecord {
        schema_version: AttestationRecord::SCHEMA_VERSION,
        record_id:      make_record_id(seq),
        created_tick:   0,
        profile:        AttestationProfile::FjellLocalV1Binary,
        nonce:          None,
        boot: BootClaims {
            selected_slot: 0, boot_id: 1,
            kernel_digest: Digest32([0x11; 32]),
        },
        verification: VerificationClaims {
            release_digest:   Digest32([0x22; 32]),
            rootfs_digest:    Digest32([0x33; 32]),
            policy_digest:    Digest32([0x44; 32]),
            release_verified: true, rootfs_verified: true, policy_verified: true,
        },
        measurement: MeasurementClaims {
            head_seq:          meas_seq,
            chain_digest:      Digest32([0x55; 32]),
            included_from_seq: 1,
            included_to_seq:   meas_seq,
        },
        snapshot:  SnapshotClaims  { snapshot_id: *b"SN000001", snapshot_digest: Digest32([0x66; 32]), reason: 0 },
        health:    HealthClaims    { target: *b"attestd\0", status: 0 },
        freshness: FreshnessClaims { generation: 1, key_epoch: 1, status: 0 },
        provenance: None,
    };
    let signed = SignedAttestationRecord::sign(record);
    (signed.record_digest, signed.verify())
}

// ── Entry point ──────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    let provider = DevelopmentTrustProvider::with_default_key(ATTESTD_PROVIDER_ID, 1);
    let mut registry = ProviderRegistry::new();
    let _ = registry.register(dev_descriptor());
    registry.enter_enforcing();
    let keyring = init_keyring();

    // In-process rollback record for the dev channel.
    let rollback = RollbackRecord::genesis(DEV_CHANNEL);

    sys_debug_writeln("attestd: trust-provider + keyring wired; v2 signing active");
    send_ready();
    sys_debug_writeln("attestd ready");

    let mut record_seq: u32  = 0;
    let mut generation: u32  = 0;
    let mut last_ok:    bool = false;
    let mut last_digest      = Digest32::ZERO;

    loop {
        let (tag_packed, w0, _w1) = recv_call();
        let tag = tag_packed & 0xFFFF;
        match tag {
            proto::GENERATE => {
                record_seq = record_seq.wrapping_add(1);
                generation = generation.wrapping_add(1);
                let meas_seq = w0 as u64;

                // v0.3.0: produce v2 when Enforcing, v1 otherwise.
                let (digest, ok) = if registry.phase() == RegistryPhase::Enforcing {
                    sign_v2(&provider, &keyring, &rollback, record_seq, meas_seq, generation)
                } else {
                    sign_v1(record_seq, meas_seq)
                };
                last_digest = digest;
                last_ok     = ok;
                reply(proto::GENERATED);
                sys_debug_writeln("attestd: record generated (v2)");
            }
            proto::VERIFY_LATEST => {
                reply(if last_ok { proto::VERIFY_OK } else { proto::VERIFY_FAIL });
            }
            proto::EXPORT => { reply(proto::EXPORT_DONE); }
            _ => reply(proto::ERR),
        }
        let _ = last_digest;
    }
}

// ── Remote attestation push (RFC v0.4-005 §5.3) ──────────────────────────────

/// Cap slot for the `secure-transportd` endpoint.
const CAP_SXT_EP: fjell_cap::CapHandle = fjell_cap::CapHandle(5);

// SXT tag constants — must match secure-transportd.
const SXT_OPEN_CHANNEL:      u16 = 0x0100;
const SXT_OPENED:            u16 = 0x0101;
const SXT_ATTEST_PUSH:       u16 = 0x0106;
const SXT_ATTEST_CHALLENGE:  u16 = 0x0107;
const SXT_CLOSE:             u16 = 0x0109;
    #[allow(dead_code)] // v0.7: SXT channel fault state tracking
const SXT_FAULTED:           u16 = 0x010b;

/// Cached server nonce from the last successful attestation push.
/// Stored in-process (alpha.1); migrates to storaged IPC in alpha.2.
static mut CACHED_NONCE: [u8; 16] = [0u8; 16];

fn sxt_send(tag: u16, w0: usize) {
    // SAFETY: shared-memory region is capability-gated; pointer is valid for the agreed-upon length.
    unsafe {
        core::arch::asm!(
            "li a7, 20", "ecall",
            in("a0") CAP_SXT_EP.0 as usize, in("a1") tag as usize, in("a2") w0,
            lateout("a0") _, lateout("a7") _, options(nostack)
        );
    }
}

fn sxt_recv() -> (u16, usize) {
    let (mut t, mut w0) = (0usize, 0usize);
    // SAFETY: shared-memory region is capability-gated; pointer is valid for the agreed-upon length.
    unsafe {
        core::arch::asm!(
            "li a7, 21", "ecall",
            in("a0") CAP_SXT_EP.0 as usize,
            lateout("a1") t, lateout("a2") w0,
            lateout("a3") _, lateout("a4") _, lateout("a5") _, lateout("a7") _,
            options(nostack)
        );
    }
    ((t & 0xFFFF) as u16, w0)
}

/// Push the current attestation record to the remote endpoint via
/// `secure-transportd` (RFC v0.4-005 §5.3 attestation exchange).
///
/// On success updates `CACHED_NONCE` with the server's next nonce.
/// Returns `true` if the round-trip completed.
pub fn push_attestation(record_seq: u32) -> bool {
    // Open an Attestation channel (kind byte = 0x03).
    sxt_send(SXT_OPEN_CHANNEL, 0x0300_0000);
    let (reply, w0) = sxt_recv();
    if reply != SXT_OPENED { return false; }
    let channel_id = w0 as u32;

    // Push the signed attestation record (w0 = record sequence number).
    sxt_send(SXT_ATTEST_PUSH, record_seq as usize);
    let (ack, nonce_lo) = sxt_recv();

    sxt_send(SXT_CLOSE, channel_id as usize);

    if ack == SXT_ATTEST_CHALLENGE {
        // Cache the returned nonce (lower 8 bytes from w0 in alpha.1 stub).
        // SAFETY: shared-memory region is capability-gated; pointer is valid for the agreed-upon length.
        unsafe {
            let b = (nonce_lo as u64).to_le_bytes();
            CACHED_NONCE[..8].copy_from_slice(&b);
        }
        true
    } else {
        false
    }
}
