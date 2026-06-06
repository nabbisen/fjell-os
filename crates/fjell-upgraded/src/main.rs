//! upgraded — Immutable A/B upgrade staging for Fjell OS.
//!
//! v0.3.0: enforces the anti-rollback counter policy (RFC v0.3-003).
//! Reads the persisted `RollbackRecord` (represented in-process as
//! `min_counter` since storaged IPC is complete in v0.4) and rejects any
//! candidate whose `release_counter` is below the floor.
#![no_std]
#![no_main]
mod rt;

use fjell_syscall::{sys_debug_writeln, sys_exit};

use fjell_trust_provider::descriptor::TrustProviderDescriptor;
use fjell_trust_provider::development::DevelopmentTrustProvider;
use fjell_trust_provider::ids::TrustProviderId;
use fjell_trust_provider::profile::{
    TrustProviderCapabilities, TrustProviderKind, TrustProfile, TrustProviderState,
};
use fjell_trust_provider::provider::HardwareTrustProvider;
use fjell_trust_provider::registry::ProviderRegistry;

use fjell_upgrade_format::rollback_record::{
    AdvanceSource, RollbackRecord, RollbackCheckResult, check_rollback, advance_min_counter,
};
use fjell_upgrade_format::release_metadata::{Provenance, ReleaseMetadata};

use fjell_keyring::KeyEpoch;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    sys_debug_writeln("upgraded: panic");
    sys_exit(1);
}

const UPGRADED_PROVIDER_ID: TrustProviderId = TrustProviderId::new(0x09);

fn dev_descriptor() -> TrustProviderDescriptor {
    TrustProviderDescriptor::new(
        UPGRADED_PROVIDER_ID,
        TrustProviderKind::Development,
        TrustProfile::FjellLocalV1,
        TrustProviderCapabilities::DEVELOPMENT_BASELINE,
        TrustProviderState::Active,
        1,
        *b"upgraded",
    )
}

// ── Anti-rollback enforcement ─────────────────────────────────────────────────

/// In v0.3.0 the rollback record is held in memory (no storaged IPC yet).
/// v0.4 RFC 004 will persist this through the IPC path.
struct AntiRollbackState {
    record: RollbackRecord,
}

impl AntiRollbackState {
    fn new(channel_id: [u8; 8]) -> Self {
        Self { record: RollbackRecord::genesis(channel_id) }
    }

    /// Attempt to confirm `meta`.  Returns `true` on success.
    fn confirm(&mut self, meta: &ReleaseMetadata, provider: &DevelopmentTrustProvider) -> bool {
        // Read trust-provider counter as a soft binding.
        let _tp_ctr = provider.read_anti_rollback_counter().unwrap_or(0);

        match check_rollback(
            self.record.min_counter,
            meta.release_counter,
            meta.embedded_min_counter,
        ) {
            RollbackCheckResult::Allowed => {
                let new_min = advance_min_counter(
                    self.record.min_counter,
                    meta.release_counter,
                );
                self.record = RollbackRecord::new(
                    meta.channel_id,
                    new_min,
                    0, // tick: 0 in dev mode
                    AdvanceSource::UpgradedConfirmation,
                );
                sys_debug_writeln("upgraded: anti-rollback confirmed; min_counter advanced");
                true
            }
            RollbackCheckResult::Rejected { .. } => {
                sys_debug_writeln("upgraded: anti-rollback REJECTED — counter below floor");
                false
            }
            RollbackCheckResult::MetadataInconsistent => {
                sys_debug_writeln("upgraded: anti-rollback REJECTED — metadata inconsistent");
                false
            }
        }
    }
}

// ── Entry point ──────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("upgraded: starting");

    let provider = DevelopmentTrustProvider::with_default_key(UPGRADED_PROVIDER_ID, 1);
    let mut registry = ProviderRegistry::new();
    let _ = registry.register(dev_descriptor());
    registry.enter_enforcing();

    // In v0.3.0 a single well-known dev channel is used.
    const DEV_CHANNEL: [u8; 8] = *b"dev\0\0\0\0\0";
    let mut arb = AntiRollbackState::new(DEV_CHANNEL);

    // Simulate staging release counter=1 (first install — always succeeds).
    let meta_1 = ReleaseMetadata::dev(DEV_CHANNEL, 1);
    if !arb.confirm(&meta_1, &provider) {
        sys_debug_writeln("upgraded: unexpected rejection on first install");
        sys_exit(1);
    }
    sys_debug_writeln("upgraded: release 1 confirmed (min_counter=1)");

    // Simulate staging release counter=2 (forward update — succeeds).
    let meta_2 = ReleaseMetadata::dev(DEV_CHANNEL, 2);
    if !arb.confirm(&meta_2, &provider) {
        sys_debug_writeln("upgraded: unexpected rejection on forward update");
        sys_exit(1);
    }
    sys_debug_writeln("upgraded: release 2 confirmed (min_counter=2)");

    // Simulate a rollback attempt: counter=1 below min_counter=2 — must fail.
    let meta_old = ReleaseMetadata::dev(DEV_CHANNEL, 1);
    if arb.confirm(&meta_old, &provider) {
        sys_debug_writeln("upgraded: expected rollback rejection was NOT raised");
        sys_exit(1);
    }
    sys_debug_writeln("upgraded: rollback correctly rejected");

    sys_debug_writeln("upgraded: boot confirmation simulated");
    sys_exit(0)
}

// ── Remote metadata fetch (RFC v0.4-004) ─────────────────────────────────────

/// Cap slot for the secure-transportd endpoint.
use fjell_cap::CapHandle;
const CAP_SXT_EP: CapHandle = CapHandle(6);

/// SXT IPC tag constants (must match secure-transportd).
const SXT_OPEN_CHANNEL:         u16 = 0x0100;
const SXT_UPDATE_METADATA_FETCH:u16 = 0x0102;
const SXT_CLOSE:                u16 = 0x0109;
const SXT_OPENED:               u16 = 0x0101;
const SXT_UPDATE_METADATA_REPLY:u16 = 0x0103;
const SXT_FAULTED:              u16 = 0x010b;

fn send_sxt(tag: u16, w0: usize) {
    // SAFETY: slot pointer is valid within the BCB; access serialised by the upgrade-lock capability.
    unsafe {
        core::arch::asm!(
            "li a7, 20", "ecall",
            in("a0") CAP_SXT_EP.0 as usize, in("a1") tag as usize, in("a2") w0,
            lateout("a0") _, lateout("a7") _, options(nostack)
        );
    }
}

fn recv_sxt() -> (u16, usize) {
    let (mut t, mut w0) = (0usize, 0usize);
    // SAFETY: slot pointer is valid within the BCB; access serialised by the upgrade-lock capability.
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

/// Fetch the remote update index over a `secure-transportd` channel.
///
/// Returns `Some(channel_id)` on success so the caller can issue the actual
/// `UPDATE_METADATA_FETCH` IPC, or `None` on handshake failure.
///
/// RFC v0.4-004 §5.1: channel kind = 0x01 (UpdateMetadata).
pub fn fetch_update_index() -> Option<u32> {
    // Request channel open: kind = UpdateMetadata (0x01).
    send_sxt(SXT_OPEN_CHANNEL, 0x0100_0000);
    let (reply_tag, w0) = recv_sxt();
    if reply_tag != SXT_OPENED {
        return None;
    }
    let channel_id = w0 as u32;

    // Issue metadata fetch on the established channel.
    send_sxt(SXT_UPDATE_METADATA_FETCH, channel_id as usize);
    let (reply_tag2, _w0_2) = recv_sxt();
    if reply_tag2 != SXT_UPDATE_METADATA_REPLY {
        send_sxt(SXT_CLOSE, channel_id as usize);
        return None;
    }

    // Caller would read the payload from a shared memory page; here we return
    // the channel_id so the integration test can verify the round-trip.
    Some(channel_id)
}
