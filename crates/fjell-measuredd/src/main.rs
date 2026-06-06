//! measuredd — Measurement chain service for Fjell OS.
//!
//! v0.3.0-alpha.1: calls `set_measurement()` on the trust provider after
//! every chain append, keeping the provider's chain-head view current for
//! attestation (RFC v0.3-001 §5.3).
#![no_std]
#![no_main]
mod rt;

use fjell_measure_format::{
    Digest32, MeasurementEvent, MeasurementHead, MeasurementKind,
    MeasurementSource, MeasurementSubject, MeasurementError,
};
use fjell_service_api::measuredd as proto;
use fjell_syscall::{sys_debug_writeln, sys_exit};
use fjell_trust_provider::descriptor::TrustProviderDescriptor;
use fjell_trust_provider::development::DevelopmentTrustProvider;
use fjell_trust_provider::ids::TrustProviderId;
use fjell_trust_provider::profile::{
    TrustProviderCapabilities, TrustProviderKind, TrustProfile, TrustProviderState,
};
use fjell_trust_provider::registry::ProviderRegistry;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    sys_debug_writeln("measuredd: panic");
    sys_exit(1);
}

// ── IPC helpers ─────────────────────────────────────────────────────────────

const EP_SLOT: u32 = 0;

fn send_ready() {
    // SAFETY: measurement chain ring is exclusively owned by this service via the MeasurementDrain cap.
    unsafe {
        core::arch::asm!(
            "li a7, 20", "ecall",
            in("a0") EP_SLOT as usize, in("a1") proto::READY,
            lateout("a0") _, lateout("a7") _, options(nostack)
        );
    }
}

fn recv_call() -> (usize, usize, usize, usize, usize) {
    let (mut t, mut w0, mut w1, mut w2, mut w3) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
    // SAFETY: measurement chain ring is exclusively owned by this service via the MeasurementDrain cap.
    unsafe {
        core::arch::asm!(
            "li a7, 21", "ecall",
            in("a0") EP_SLOT as usize,
            lateout("a1") t, lateout("a2") w0, lateout("a3") w1,
            lateout("a4") w2, lateout("a5") w3,
            lateout("a7") _, options(nostack)
        );
    }
    (t, w0, w1, w2, w3)
}

fn reply(tag: usize, w0: usize, w1: usize, w2: usize) {
    // SAFETY: measurement chain ring is exclusively owned by this service via the MeasurementDrain cap.
    unsafe {
        core::arch::asm!(
            "li a7, 23", "ecall",
            in("a0") 0usize, in("a1") tag,
            in("a2") w0, in("a3") w1, in("a4") w2,
            lateout("a7") _, options(nostack)
        );
    }
}

// ── Measurement chain ────────────────────────────────────────────────────────

const MAX_EVENTS: usize = 64;

struct Chain {
    events:   [Option<MeasurementEvent>; MAX_EVENTS],
    head:     MeasurementHead,
    next_seq: u64,
}

impl Chain {
    const fn new() -> Self {
        Self { events: [None; MAX_EVENTS], head: MeasurementHead::EMPTY, next_seq: 1 }
    }

    fn append(
        &mut self,
        kind:    MeasurementKind,
        source:  MeasurementSource,
        subject: MeasurementSubject,
        sd:      Digest32,
        meta:    Option<Digest32>,
    ) -> Result<(u64, Digest32), MeasurementError> {
        let seq  = self.next_seq;
        let prev = self.head.chain_digest;
        let ev   = MeasurementEvent::new(seq, 0, kind, source, subject, sd, meta, prev);
        self.events[((seq - 1) as usize) % MAX_EVENTS] = Some(ev);
        self.head = MeasurementHead {
            latest_seq: seq, chain_digest: ev.chain_digest,
            dropped: self.head.dropped, last_event_kind: kind,
        };
        self.next_seq = seq + 1;
        Ok((seq, ev.chain_digest))
    }
}

fn decode_kind(b: u8) -> MeasurementKind {
    match b {
        0x02 => MeasurementKind::ReleaseManifestVerified,
        0x03 => MeasurementKind::RootfsManifestVerified,
        0x04 => MeasurementKind::PolicyBundleVerified,
        0x07 => MeasurementKind::SnapshotCreated,
        0x08 => MeasurementKind::HealthTargetResult,
        0x09 => MeasurementKind::BundleFreshnessChecked,
        0x0A => MeasurementKind::RecoveryTargetEntered,
        0x0C => MeasurementKind::AttestationGenerated,
        _    => MeasurementKind::BootEvidenceImported,
    }
}

fn decode_source(b: u8) -> MeasurementSource {
    match b {
        0x02 => MeasurementSource::Verifyd,
        0x07 => MeasurementSource::Snapshotd,
        0x09 => MeasurementSource::Upgraded,
        0x0A => MeasurementSource::Recoveryd,
        0x0B => MeasurementSource::Attestd,
        _    => MeasurementSource::Measuredd,
    }
}

fn decode_subject(b: u8) -> MeasurementSubject {
    match b {
        0x02 => MeasurementSubject::ReleaseManifest,
        0x03 => MeasurementSubject::RootfsManifest,
        0x04 => MeasurementSubject::PolicyBundle,
        0x07 => MeasurementSubject::SystemSnapshot,
        0x08 => MeasurementSubject::HealthResult,
        0x09 => MeasurementSubject::BundleMetadata,
        0x0A => MeasurementSubject::RecoveryAction,
        0x0B => MeasurementSubject::AttestationRecord,
        _    => MeasurementSubject::BootEvidence,
    }
}

// ── Trust-provider init ──────────────────────────────────────────────────────

const MEASUREDD_PROVIDER_ID: TrustProviderId = TrustProviderId::new(0x02);

fn dev_descriptor() -> TrustProviderDescriptor {
    TrustProviderDescriptor::new(
        MEASUREDD_PROVIDER_ID,
        TrustProviderKind::Development,
        TrustProfile::FjellLocalV1,
        TrustProviderCapabilities::DEVELOPMENT_BASELINE,
        TrustProviderState::Active,
        1,
        *b"measrdd\0",
    )
}

// ── Entry point ──────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    let mut chain = Chain::new();

    let provider = DevelopmentTrustProvider::with_default_key(MEASUREDD_PROVIDER_ID, 1);
    let mut registry = ProviderRegistry::new();
    let _ = registry.register(dev_descriptor());
    registry.enter_enforcing();
    sys_debug_writeln("measuredd: trust-provider registered (dev)");

    send_ready();
    sys_debug_writeln("measuredd ready");

    loop {
        let (tag_packed, w0, w1, w2, _w3) = recv_call();
        let tag = tag_packed & 0xFFFF;
        match tag {
            proto::APPEND_EVENT => {
                let kind    = decode_kind   (((w0 >> 24) & 0xFF) as u8);
                let source  = decode_source (((w0 >> 16) & 0xFF) as u8);
                let subject = decode_subject(((w0 >>  8) & 0xFF) as u8);
                let mut sd  = [0u8; 32];
                sd[0..8].copy_from_slice(&w1.to_le_bytes());
                sd[8..16].copy_from_slice(&w2.to_le_bytes());
                match chain.append(kind, source, subject, Digest32(sd), None) {
                    Ok((seq, cd)) => {
                        // v0.3: keep provider chain-head view current.
                        provider.set_measurement(chain.head);
                        let cd_lo = u64::from_le_bytes(
                            cd.0[0..8].try_into().unwrap_or([0; 8]),
                        );
                        reply(proto::APPEND_OK, seq as usize, cd_lo as usize, 0);
                    }
                    Err(_) => reply(proto::ERR, MeasurementError::Internal as usize, 0, 0),
                }
            }
            proto::GET_HEAD => {
                let h     = chain.head;
                let cd_lo = u64::from_le_bytes(
                    h.chain_digest.0[0..8].try_into().unwrap_or([0; 8]),
                );
                reply(proto::HEAD_REPLY, h.latest_seq as usize, cd_lo as usize,
                      h.dropped as usize);
            }
            proto::EXPORT_LOG => {
                reply(proto::EXPORT_CHUNK, chain.head.latest_seq as usize, 0, 0);
                reply(proto::EXPORT_DONE, 0, 0, 0);
            }
            _ => reply(proto::ERR, MeasurementError::Internal as usize, 0, 0),
        }
    }
}
