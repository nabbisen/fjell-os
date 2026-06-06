//! verifyd — Signature verification service for Fjell OS.
//!
//! v0.3.0-alpha.1: registers a `DevelopmentTrustProvider`, initialises a dev
//! `Keyring` with a `DevDigest32` anchor, and verifies signatures via
//! `DevSignatureProvider` (RFC v0.3-002 §5.3).
#![allow(unused_assignments)]  // IPC polling idiom: t/w* are overwritten by sys_ipc_recv
#![no_std]
#![no_main]
mod rt;

use fjell_syscall::{sys_debug_writeln, sys_exit};

use fjell_keyring::algorithm::SignatureAlgorithm;
use fjell_keyring::anchor::{AuthorityClass, TrustAnchor};
use fjell_keyring::dev_provider::DevSignatureProvider;
use fjell_keyring::epoch::KeyEpoch;
use fjell_keyring::KeyPurpose;
use fjell_keyring::keyring::Keyring;
use fjell_keyring::provider::SignatureProvider;

use fjell_trust_provider::descriptor::TrustProviderDescriptor;
use fjell_trust_provider::development::DevelopmentTrustProvider;
use fjell_trust_provider::ids::TrustProviderId;
use fjell_trust_provider::profile::{
    TrustProviderCapabilities, TrustProviderKind, TrustProfile, TrustProviderState,
};
use fjell_trust_provider::registry::ProviderRegistry;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    sys_debug_writeln("verifyd: panic");
    sys_exit(1);
}

// ── IPC tags ─────────────────────────────────────────────────────────────────

mod proto {
    pub const READY:       usize = 0x0001;
    pub const VERIFY:      usize = 0x0010;
    pub const VERIFY_OK:   usize = 0x0011;
    pub const VERIFY_FAIL: usize = 0x0012;
    pub const ERR:         usize = 0xFFFF;
}

// ── IPC helpers ─────────────────────────────────────────────────────────────

const EP_SLOT: u32 = 0;

fn send_ready() {
    // SAFETY: category=user-copy shared-memory region is capability-gated; pointer is valid for the agreed-upon length.
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
    // SAFETY: category=user-copy shared-memory region is capability-gated; pointer is valid for the agreed-upon length.
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
    // SAFETY: category=user-copy shared-memory region is capability-gated; pointer is valid for the agreed-upon length.
    unsafe {
        core::arch::asm!(
            "li a7, 23", "ecall",
            in("a0") 0usize, in("a1") tag,
            lateout("a7") _, options(nostack)
        );
    }
}

// ── Trust-provider / keyring init ────────────────────────────────────────────

const VERIFYD_PROVIDER_ID: TrustProviderId = TrustProviderId::new(0x03);
const DEV_ANCHOR_KEY: [u8; 32] = [0u8; 32];

fn dev_descriptor() -> TrustProviderDescriptor {
    TrustProviderDescriptor::new(
        VERIFYD_PROVIDER_ID,
        TrustProviderKind::Development,
        TrustProfile::FjellLocalV1,
        TrustProviderCapabilities::DEVELOPMENT_BASELINE,
        TrustProviderState::Active,
        1,
        *b"verifyd\0",
    )
}

fn init_keyring() -> Keyring {
    let mut keyring = Keyring::new();
    if let Some(anchor_rv) = TrustAnchor::new(
        KeyPurpose::ReleaseVerification,
        SignatureAlgorithm::DevDigest32,
        AuthorityClass::Genesis,
        KeyEpoch::ONE,
        &DEV_ANCHOR_KEY,
    ) {
        let _ = keyring.install(anchor_rv);
    }
    if let Some(anchor_as) = TrustAnchor::new(
        KeyPurpose::AttestationSigning,
        SignatureAlgorithm::DevDigest32,
        AuthorityClass::Genesis,
        KeyEpoch::ONE,
        &DEV_ANCHOR_KEY,
    ) {
        let _ = keyring.install(anchor_as);
    }
    keyring
}

// ── Verification ─────────────────────────────────────────────────────────────

/// Verify a dev `DevDigest32` signature.
///
/// `digest_lo` and `sig_lo` are the low-8-byte encodings of the full 32-byte
/// values; the upper bytes are zero-padded.  Full shared-memory transfer of
/// 32-byte digests lands in v0.3.0.
fn verify_dev(keyring: &Keyring, digest_lo: u64, sig_lo: u64) -> bool {
    let mut digest_b = [0u8; 32];
    let mut sig_b    = [0u8; 32];
    digest_b[0..8].copy_from_slice(&digest_lo.to_le_bytes());
    sig_b   [0..8].copy_from_slice(&sig_lo   .to_le_bytes());

    let anchor = match keyring.anchors_for(KeyPurpose::ReleaseVerification).next() {
        Some(a) => a,
        None    => return false,
    };

    DevSignatureProvider.verify(&anchor, &digest_b, &sig_b).is_ok()
}

// ── Entry point ──────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    let _provider = DevelopmentTrustProvider::with_default_key(VERIFYD_PROVIDER_ID, 1);
    let mut registry = ProviderRegistry::new();
    let _ = registry.register(dev_descriptor());
    registry.enter_enforcing();
    let keyring = init_keyring();
    sys_debug_writeln("verifyd: keyring wired (dev, ReleaseVerification + AttestationSigning)");

    send_ready();
    sys_debug_writeln("verifyd ready");

    loop {
        let (tag_packed, w0, w1) = recv_call();
        let tag = tag_packed & 0xFFFF;
        match tag {
            proto::VERIFY => {
                if verify_dev(&keyring, w0 as u64, w1 as u64) {
                    reply(proto::VERIFY_OK);
                } else {
                    reply(proto::VERIFY_FAIL);
                }
            }
            _ => reply(proto::ERR),
        }
    }
}
