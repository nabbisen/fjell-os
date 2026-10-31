//! attestd — Local attestation service for Fjell OS.
//!
//! v0.3.0: produces `AttestationRecordV2` when the trust-provider registry
//! is in Enforcing phase (RFC v0.3-004).  Falls back to the v1 dev path
//! when still in Bootstrap (development / early boot).
#![no_std]
#![no_main]
mod rt;

// RFC-0.37-002 (closes E-082): narrowed from a crate-level
// `#![allow(unused_assignments, unused_imports, dead_code)]` -- re-derived,
// only `unused_imports` still fires, and only for these five names.
// `unused_assignments` and `dead_code` suppress nothing today (confirmed by
// deleting the blanket and building clean).
#[allow(unused_imports)]
// M7 stub: v2 claim types this service will construct once Enforcing-phase signing lands
use fjell_attestation_format::v2::{
    AttestationRecordV2, FreshnessClaimsV2, KeyringClaims, NonceClass, ProviderClaims,
    RollbackClaims, SignedAttestationRecordV2, SignedByDescriptor,
};
use fjell_attestation_format::{
    AttestationProfile, AttestationRecord, AttestationRecordId, BootClaims, FreshnessClaims,
    HealthClaims, MeasurementClaims, SignedAttestationRecord, SnapshotClaims, VerificationClaims,
};
use fjell_cap::CapHandle;
use fjell_measure_format::Digest32;
use fjell_service_api::attestd as proto;
use fjell_syscall::{sys_debug_writeln, sys_exit};

use fjell_trust_provider::descriptor::TrustProviderDescriptor;
use fjell_trust_provider::development::DevelopmentTrustProvider;
use fjell_trust_provider::ids::TrustProviderId;
#[allow(unused_imports)] // v0.7: hardware attestation material
use fjell_trust_provider::material::AttestationDigest;
use fjell_trust_provider::profile::{
    TrustProfile, TrustProviderCapabilities, TrustProviderKind, TrustProviderState,
};
use fjell_trust_provider::provider::HardwareTrustProvider;
use fjell_trust_provider::registry::{ProviderRegistry, RegistryPhase};

use fjell_keyring::KeyPurpose;
use fjell_keyring::algorithm::SignatureAlgorithm;
use fjell_keyring::anchor::{AuthorityClass, TrustAnchor};
use fjell_keyring::epoch::KeyEpoch;
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

/// RFC-0.40-001 R4: a `CALL` capability to `measuredd`'s endpoint, so the record's
/// measurement section can be the chain's real head. `CALL` and nothing else.
/// Reading the head needs only the endpoint, but this service must not be able to
/// WRITE the chain: that is refused by `measuredd` (`ATTESTD` is not a source it
/// maps, RFC-0.40-001 R5), not by this capability, which cannot tell a read from
/// an append -- so the two changes land together.
const CAP_MEASUREDD_EP: CapHandle = CapHandle(4);

fn send_ready() {
    // RFC-0.28-001: previously sent `proto::READY` to this service's own
    // endpoint (`EP_SLOT`), which only `init`'s direct `wait_service_ready`
    // ever received. `init` no longer holds a receive capability there
    // (narrowed to `CALL`); readiness now goes through the generic
    // `tags::SERVICE_READY` protocol to service-manager's dedicated
    // endpoint, which relays it on to `init`.
    // RFC-0.28-002: was a hand-rolled asm block; now the audited wrapper.
    let _ = fjell_syscall::sys_ipc_send(
        fjell_service_api::ready::SERVICE_READY_SEND_SLOT,
        fjell_service_api::tags::SERVICE_READY,
    );
}

fn recv_call() -> (usize, usize, usize) {
    // RFC-0.28-002: was a hand-rolled `IpcRecv` asm block; `sys_ipc_recv_msg`
    // is a correct superset (`w2`/`w3`/sender discarded, not needed here).
    match fjell_syscall::sys_ipc_recv_msg(EP_SLOT) {
        Ok((t, w0, w1, _w2, _w3, _sender)) => (t, w0, w1),
        Err(_) => (0, 0, 0),
    }
}

fn reply(tag: usize) {
    // RFC-0.28-002: was a hand-rolled asm block; now the audited wrapper.
    let _ = fjell_syscall::sys_ipc_reply(tag);
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
    id[0] = b'A';
    id[1] = b'T';
    let mut n = seq;
    let mut i = 7usize;
    while n > 0 && i >= 2 {
        id[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if i > 2 {
            i -= 1;
        } else {
            break;
        }
    }
    AttestationRecordId(id)
}

/// Print `n` in decimal (no allocation).
fn print_dec(mut n: u64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    if let Ok(s) = core::str::from_utf8(&buf[i..]) {
        fjell_syscall::sys_debug_write(s);
    }
}

/// RFC-0.40-001 R6: say what the record being signed CLAIMS, read out of the record
/// itself with the same accessors the encoder uses, so a tier can compare it with
/// what is true. (Nothing outside this service could read the record before:
/// `GENERATE` returns only a digest and `EXPORT` is a stub.)
fn print_record_summary(record: &AttestationRecordV2) {
    let a = &record.availability;
    fjell_syscall::sys_debug_write("attestd: record verified_flags=");
    print_dec(record.verified_flags() as u64);
    sys_debug_writeln("");
    fjell_syscall::sys_debug_write("attestd: record availability kernel_digest=");
    print_dec(a.kernel_digest as u64);
    fjell_syscall::sys_debug_write(" release_digest=");
    print_dec(a.release_digest as u64);
    fjell_syscall::sys_debug_write(" rootfs_digest=");
    print_dec(a.rootfs_digest as u64);
    fjell_syscall::sys_debug_write(" policy_digest=");
    print_dec(a.policy_digest as u64);
    fjell_syscall::sys_debug_write(" snapshot_digest=");
    print_dec(a.snapshot_digest as u64);
    fjell_syscall::sys_debug_write(" measurement=");
    print_dec(a.measurement as u64);
    sys_debug_writeln("");
    // RFC-0.41-003 R1 (E-101): the placeholders, each behind its own disclosure. A `0`
    // here means the value in the record is NOT a reading -- it is written as zero.
    fjell_syscall::sys_debug_write("attestd: record availability keyring_epochs=");
    print_dec(a.keyring_epochs as u64);
    fjell_syscall::sys_debug_write(" snapshot_id=");
    print_dec(a.snapshot_id as u64);
    fjell_syscall::sys_debug_write(" health_status=");
    print_dec(a.health_status as u64);
    fjell_syscall::sys_debug_write(" last_advance_source=");
    print_dec(a.last_advance_source as u64);
    fjell_syscall::sys_debug_write(" freshness_key_epoch=");
    print_dec(a.freshness_key_epoch as u64);
    sys_debug_writeln("");
    let m = record.measurement_written();
    fjell_syscall::sys_debug_write("attestd: record measurement head_seq=");
    print_dec(m.head_seq);
    fjell_syscall::sys_debug_write(" included=");
    print_dec(m.included_from_seq);
    fjell_syscall::sys_debug_write("..");
    print_dec(m.included_to_seq);
    sys_debug_writeln("");
    let mut hex = [0u8; 64];
    fjell_syscall::sys_debug_write("attestd: record measurement chain_digest sha256:");
    sys_debug_writeln(m.chain_digest.to_hex(&mut hex));
    fjell_syscall::sys_debug_write("attestd: record rollback counter_supported=");
    print_dec(record.rollback.trust_provider_counter_supported as u64);
    fjell_syscall::sys_debug_write(" counter_value=");
    print_dec(record.rollback.trust_provider_counter_value);
    sys_debug_writeln("");
}

/// The chain as `measuredd` reports it, as the record's measurement section --
/// or `None` if it cannot be read. **`None` makes the section unavailable**; there
/// is no fallback to a stale or literal head (that would be E-094 with extra steps).
fn read_measurement() -> Option<MeasurementClaims> {
    let r = fjell_service_api::measuredd::read_chain(CAP_MEASUREDD_EP.0)?;
    // An empty chain has no head to attest, and `1..0` is not a range.
    if r.events == 0 {
        return None;
    }
    Some(MeasurementClaims {
        head_seq: r.events,
        chain_digest: Digest32(r.head),
        included_from_seq: 1,
        included_to_seq: r.events,
    })
}

/// Sign a v2 record when the registry is in Enforcing phase.
///
/// RFC-0.40-001: this used to take `_meas_seq: u64`, commented "forwarded to
/// measurement chain header" and never read. It takes the chain reading instead --
/// the real head, or `None` for "could not be read" -- because a sequence number a
/// caller passes in is a claim, and this service now reads the chain itself.
fn sign_v2(
    provider: &DevelopmentTrustProvider,
    keyring: &Keyring,
    rollback: &RollbackRecord,
    seq: u32,
    generation: u32,
    measurement: Option<MeasurementClaims>,
) -> (Digest32, bool) {
    // F1 (fix-forward 0.40): the record's `trust_provider_counter_*` pair says whether
    // THIS call succeeded and what it returned, so it is made here, once per record
    // -- never cached, because the counter advances. `Err` is not `0` with the flag
    // still true: it is "unsupported", and `local()` writes flag 0 and value 0.
    let provider_counter = provider.read_anti_rollback_counter().ok();
    let record = AttestationRecordV2::local(
        make_record_id(seq),
        ATTESTD_PROVIDER_ID,
        DEV_CHANNEL,
        [0u8; 16], // dev nonce
        rollback.min_counter,
        provider_counter,
        generation,
        measurement,
    );
    print_record_summary(&record);

    // Build signed_by from the keyring.
    let epoch = keyring
        .anchors_for(KeyPurpose::AttestationSigning)
        .next()
        .map(|a| a.epoch.raw())
        .unwrap_or(1);

    let signed_by = SignedByDescriptor {
        provider_id: ATTESTD_PROVIDER_ID,
        provider_generation: 1,
        keyring_anchor_epoch: epoch,
        algorithm: SignatureAlgorithm::DevDigest32 as u8,
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
        record_id: make_record_id(seq),
        created_tick: 0,
        profile: AttestationProfile::FjellLocalV1Binary,
        nonce: None,
        boot: BootClaims {
            selected_slot: 0,
            boot_id: 1,
            kernel_digest: Digest32([0x11; 32]),
        },
        verification: VerificationClaims {
            release_digest: Digest32([0x22; 32]),
            rootfs_digest: Digest32([0x33; 32]),
            policy_digest: Digest32([0x44; 32]),
            // RFC-0.40-001: nothing verifies a release, a rootfs or a policy bundle,
            // so no path of this service signs that one was. (The rest of this v1
            // record is untouched and unreachable: `enter_enforcing()` is
            // unconditional, so only `sign_v2` runs.)
            release_verified: false,
            rootfs_verified: false,
            policy_verified: false,
        },
        measurement: MeasurementClaims {
            head_seq: meas_seq,
            chain_digest: Digest32([0x55; 32]),
            included_from_seq: 1,
            included_to_seq: meas_seq,
        },
        snapshot: SnapshotClaims {
            snapshot_id: *b"SN000001",
            snapshot_digest: Digest32([0x66; 32]),
            reason: 0,
        },
        health: HealthClaims {
            target: *b"attestd\0",
            status: 0,
        },
        freshness: FreshnessClaims {
            generation: 1,
            key_epoch: 1,
            status: 0,
        },
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

    let mut record_seq: u32 = 0;
    let mut generation: u32 = 0;
    let mut last_ok: bool = false;
    let mut last_digest = Digest32::ZERO;

    loop {
        let (tag_packed, w0, _w1) = recv_call();
        let tag = tag_packed & 0xFFFF;
        match tag {
            proto::GENERATE => {
                // RFC-0.40-001: GENERATE takes no words. It used to take the
                // measurement sequence from `init`, which `sign_v2` never read; this
                // service now reads the chain itself, so a word here would be one the
                // caller believes it set and nothing uses. Refused, not ignored.
                if w0 != 0 {
                    reply(proto::ERR);
                    continue;
                }
                record_seq = record_seq.wrapping_add(1);
                generation = generation.wrapping_add(1);

                // v0.3.0: produce v2 when Enforcing, v1 otherwise.
                let (digest, ok) = if registry.phase() == RegistryPhase::Enforcing {
                    sign_v2(
                        &provider,
                        &keyring,
                        &rollback,
                        record_seq,
                        generation,
                        read_measurement(),
                    )
                } else {
                    sign_v1(record_seq, 0)
                };
                last_digest = digest;
                last_ok = ok;
                reply(proto::GENERATED);
                sys_debug_writeln("attestd: record generated (v2)");
            }
            proto::VERIFY_LATEST => {
                reply(if last_ok {
                    proto::VERIFY_OK
                } else {
                    proto::VERIFY_FAIL
                });
            }
            proto::EXPORT => {
                reply(proto::EXPORT_DONE);
            }
            _ => reply(proto::ERR),
        }
        let _ = last_digest;
    }
}

// RFC-0.35-002 (closes E-074): "Remote attestation push" (RFC v0.4-005
// §5.3) used to live here — `push_attestation`, its `CAP_SXT_EP` (slot 5),
// and the SXT send/recv helpers it alone used. It had no caller anywhere
// in the tree (re-derived, RFC-0.35-001 R8) and addressed a peer through a
// capability nobody installs: built, never wired to a caller, the mirror
// image of E-068/E-069's declared-but-not-serving shape. Deleted, not
// stubbed — serving it is RFC-0.35-002's ruling's reason 1 (bolting a path
// onto an existing comment retrofits a design to a sentence, not the other
// way around). Its own write to `CACHED_NONCE`, a `static mut`, would have
// page-faulted the moment it ran: a service's image is mapped `R | X | U`,
// never `W` (the constraint `fjell-semantic-stream`'s own module doc
// documents) — a second, previously-latent defect in the code this
// deletion removes rather than fixes.
