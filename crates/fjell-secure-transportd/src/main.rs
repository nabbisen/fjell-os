//! secure-transportd — Authenticated control-plane channel service.
//!
//! v0.4.0-alpha.1: Receives a `Session` cap from `netd` via cap-broker,
//! initialises the channel table, and enters an IPC loop handling
//! `SXT_OPEN_CHANNEL` / `SXT_CLOSE` requests from `upgraded`, `diagnosticsd`,
//! and `attestd` (RFC v0.4-003 §5.2).
//!
//! TLS 1.3 handshake using `fjell-sxt-crypto` primitives is wired in this
//! iteration; full certificate verification against the pinned trust anchor
//! lands in v0.4.0-alpha.2.
#![no_std]
#![no_main]
mod rt;
mod channel;

use fjell_syscall::{sys_debug_writeln, sys_exit};
use fjell_cap::CapHandle;
use fjell_net_format::{ChannelKind, MAX_SXT_CHANNELS};
use fjell_sxt_crypto::tls_state::{TlsHandshakeState, TlsState, SxtError as TlsSxtError};

use channel::{ChannelTable, SxtTag, SxtError, ChannelState};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    sys_debug_writeln("secure-transportd: panic");
    sys_exit(1);
}

// ── CSpace layout ─────────────────────────────────────────────────────────────
//
//   slot 0 — Session cap (from netd, via cap-broker)
//   slot 1 — Endpoint to service-manager (ready signal)
//   slot 2 — Endpoint for upgraded requests
//   slot 3 — Endpoint for diagnosticsd requests
//   slot 4 — Endpoint for attestd requests
//
const CAP_SESSION:  CapHandle = CapHandle(0);
const CAP_SMGR_EP:  CapHandle = CapHandle(1);
const CAP_UPGRAD_EP:CapHandle = CapHandle(2);
const CAP_DIAG_EP:  CapHandle = CapHandle(3);
const CAP_ATTEST_EP:CapHandle = CapHandle(4);

// ── IPC helpers ───────────────────────────────────────────────────────────────

fn send_tag(ep: CapHandle, tag: u16, w0: usize) {
    // SAFETY: category=mmio-access virtio MMIO region is mapped and exclusive to this driver context.
    unsafe {
        core::arch::asm!(
            "li a7, 20", "ecall",
            in("a0") ep.0 as usize, in("a1") tag as usize, in("a2") w0,
            lateout("a0") _, lateout("a7") _, options(nostack)
        );
    }
}

fn recv_msg(ep: CapHandle) -> (u16, usize, usize) {
    let (mut t, mut w0, mut w1) = (0usize, 0usize, 0usize);
    // SAFETY: category=mmio-access virtio MMIO region is mapped and exclusive to this driver context.
    unsafe {
        core::arch::asm!(
            "li a7, 21", "ecall",
            in("a0") ep.0 as usize,
            lateout("a1") t, lateout("a2") w0, lateout("a3") w1,
            lateout("a4") _, lateout("a5") _, lateout("a7") _, options(nostack)
        );
    }
    ((t & 0xFFFF) as u16, w0, w1)
}

// ── TLS handshake (RFC-v0.7.3-001) ───────────────────────────────────────────
//
// In release builds (default features): the simulated path is ABSENT.
// Real certificate verification against a keyring anchor is the only path.
//
// In development builds (feature = "simulated-transport"): the old simulated
// state-machine path is still available for off-device testing.

#[cfg(feature = "simulated-transport")]
fn perform_tls_handshake(_channel_id: u32, _server_name: &[u8; 64]) -> Result<u32, SxtError> {
    // DEVELOPMENT PATH ONLY — do not enable in release builds.
    let mut hs = TlsHandshakeState::new();
    hs.start().map_err(|_| SxtError::HandshakeFailed)?;
    hs.on_server_hello().map_err(|_| SxtError::HandshakeFailed)?;
    hs.on_certificate().map_err(|_| SxtError::HandshakeFailed)?;
    hs.on_cert_verify_pass(0).map_err(|_| SxtError::CertVerifyFailed)?;
    hs.on_finished().map_err(|_| SxtError::HandshakeFailed)?;
    hs.enter_app_data().map_err(|_| SxtError::HandshakeFailed)?;
    Ok(hs.anchor_epoch)
}

#[cfg(not(feature = "simulated-transport"))]
fn perform_tls_handshake(_channel_id: u32, _server_name: &[u8; 64]) -> Result<u32, SxtError> {
    // Release path: real peer certificate verification.
    // The keyring's ReleaseVerification anchor is used to verify the server cert.
    // Full implementation in v0.7.3 (RFC-v0.7.3-001); for now fails closed.
    //
    // Steps when implemented:
    //   1. Receive ServerHello over sys_net_recv (netd session).
    //   2. Receive Certificate chain.
    //   3. Verify cert against keyring::find_anchor(KeyPurpose::ReleaseVerification).
    //   4. Complete Finished exchange.
    //
    // Current behaviour: return HandshakeFailed (fail closed — no simulated trust).
    Err(SxtError::HandshakeFailed)
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("secure-transportd: starting");

    let mut channels = ChannelTable::new();

    // Notify service-manager we are ready.
    send_tag(CAP_SMGR_EP, SxtTag::Opened as u16, 0);
    sys_debug_writeln("secure-transportd: ready");

    // IPC event loop.
    // In v0.4.0-alpha.1 we poll the three client endpoints in round-robin.
    // A proper kernel-level multiplexed recv lands in v0.4.0.
    loop {
        // Poll upgraded endpoint.
        let (tag, w0, _w1) = recv_msg(CAP_UPGRAD_EP);
        match SxtTag::from_u16(tag) {
            Some(SxtTag::OpenChannel) => {
                // w0 encodes: kind (high 8 bits) | server_name_ptr (low 24 bits).
                // In v0.4.0-alpha.1 the server name is always the update server.
                let kind_raw = (w0 >> 24) as u8;
                let kind = match kind_raw {
                    0x01 => ChannelKind::UpdateMetadata,
                    0x02 => ChannelKind::Diagnostics,
                    0x03 => ChannelKind::Attestation,
                    _    => ChannelKind::UpdateMetadata,
                };
                let mut server_name = [0u8; 64];
                // Default SNI for update channel.
                let sni = b"update.fjell.example";
                server_name[..sni.len()].copy_from_slice(sni);

                match channels.open(kind, server_name) {
                    Ok(channel_id) => {
                        match perform_tls_handshake(channel_id, &server_name) {
                            Ok(epoch) => {
                                if let Some(ch) = channels.find_mut(channel_id) {
                                    ch.state        = ChannelState::Established;
                                    ch.anchor_epoch = epoch;
                                }
                                send_tag(CAP_UPGRAD_EP, SxtTag::Opened as u16, channel_id as usize);
                                sys_debug_writeln("secure-transportd: channel opened");
                            }
                            Err(_) => {
                                channels.close(channel_id);
                                send_tag(CAP_UPGRAD_EP, SxtTag::Faulted as u16, 0);
                                sys_debug_writeln("secure-transportd: handshake failed");
                            }
                        }
                    }
                    Err(_) => {
                        send_tag(CAP_UPGRAD_EP, SxtTag::Faulted as u16, SxtError::Internal as usize);
                        sys_debug_writeln("secure-transportd: channel table full");
                    }
                }
            }
            Some(SxtTag::UpdateMetadataFetch) => {
                let channel_id = w0 as u32;
                if let Some(ch) = channels.find_mut(channel_id) {
                    if ch.state == ChannelState::Established {
                        // Stub: in alpha.2 this sends an HTTP/1.1 GET over the TLS record.
                        send_tag(CAP_UPGRAD_EP, SxtTag::UpdateMetadataReply as u16, channel_id as usize);
                    } else {
                        send_tag(CAP_UPGRAD_EP, SxtTag::Faulted as u16, SxtError::ChannelFaulted as usize);
                    }
                } else {
                    send_tag(CAP_UPGRAD_EP, SxtTag::Faulted as u16, SxtError::ChannelClosed as usize);
                }
            }
            Some(SxtTag::Close) => {
                let channel_id = w0 as u32;
                channels.close(channel_id);
                send_tag(CAP_UPGRAD_EP, SxtTag::Closed as u16, channel_id as usize);
            }
            _ => {}
        }
    }
}
