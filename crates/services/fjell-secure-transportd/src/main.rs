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
// RFC-0.35-001 (closes E-072, E-070): this service does **not** exit early —
// it signals readiness and enters its IPC loop, same as every other
// service, and (since this line installed the capability the kernel never
// had) now genuinely exchanges messages with `upgraded` rather than
// busy-spinning on an empty slot. The claim that it "signals readiness and
// exits before its main loop by design" was never true of the code; it is
// removed from `v1-limitations.md`'s stub list in the same commit. What
// remains a stub: real certificate verification (`fjell-sxt-crypto`'s own
// doc comment) and the `diagnosticsd`/`attestd` legs, which RFC-0.35-002
// decides. The allows below stay — `diagnosticsd`'s and `attestd`'s slots
// are still unserved dead paths, and the round-robin comment below is one
// of them (E-069).

#![allow(
    dead_code,
    unused_variables,
    unreachable_code,
    unused_imports,
    unused_assignments,
    unused_mut
)]
#![no_std]
#![no_main]
mod channel;
mod rt;

use fjell_cap::CapHandle;
use fjell_net_format::{ChannelKind, MAX_SXT_CHANNELS};
use fjell_sxt_crypto::tls_state::{SxtError as TlsSxtError, TlsHandshakeState, TlsState};
use fjell_syscall::{sys_debug_writeln, sys_exit, sys_yield};

use channel::{ChannelState, ChannelTable, SxtError, SxtTag};

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
const CAP_SESSION: CapHandle = CapHandle(0);
const CAP_SMGR_EP: CapHandle = CapHandle(1);
const CAP_UPGRAD_EP: CapHandle = CapHandle(2);
const CAP_DIAG_EP: CapHandle = CapHandle(3);
const CAP_ATTEST_EP: CapHandle = CapHandle(4);

// ── IPC helpers ───────────────────────────────────────────────────────────────

/// RFC-0.35-001 R6 (closes E-070): `send_tag` carries a real data word
/// again, via `sys_ipc_send_words` rather than the bare, zero-word
/// `sys_ipc_send` RFC-0.34-004 left it with. That was correct when nothing
/// reached this endpoint at all (E-072: the capability did not exist, so no
/// word — carried or not — was ever going anywhere); now that
/// `spawn.rs` installs `CAP_UPGRAD_EP`/`upgraded`'s `CAP_SXT_EP` to one
/// shared object (RFC-0.35-001 R5), the channel id this function's callers
/// pass is the fact that makes `UpdateMetadataFetch`/`Close` address the
/// right channel instead of channel `0` (never valid — see
/// `ChannelTable::find_mut`).
fn send_tag(ep: CapHandle, tag: u16, w0: usize) {
    let _ = fjell_syscall::sys_ipc_send_words(ep.0, tag as usize, w0, 0);
}

fn recv_msg(ep: CapHandle) -> (u16, usize, usize) {
    // RFC-0.28-002: was a hand-rolled `IpcRecv` asm block; `sys_ipc_recv_msg`
    // is a correct superset (`w2`/`w3`/sender discarded, not needed here).
    match fjell_syscall::sys_ipc_recv_msg(ep.0) {
        Ok((t, w0, w1, _w2, _w3, _sender)) => ((t & 0xFFFF) as u16, w0, w1),
        Err(_) => {
            // RFC-0.35-001 (closes E-072): `sys_ipc_recv` errors instead of
            // blocking on a capability slot the kernel never installed
            // (`InvalidCap`) — this loop's only call site (`CAP_UPGRAD_EP`,
            // this service polls nothing else despite its own comment
            // claiming otherwise, E-069) used to re-enter at once on every
            // such error, busy-spinning for the lifetime of every boot
            // (E-072's own measurement: 27 recorded boots, never a single
            // real message). `CAP_UPGRAD_EP` is installed by this line, so
            // this branch is not expected to fire in a committed tier
            // anymore — kept, and still yielding, as the general case
            // rather than one tied to a specific slot: a recv error must
            // not be a tight loop, whatever caused it.
            sys_yield();
            (0, 0, 0)
        }
    }
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
    hs.on_server_hello()
        .map_err(|_| SxtError::HandshakeFailed)?;
    hs.on_certificate().map_err(|_| SxtError::HandshakeFailed)?;
    hs.on_cert_verify_pass(0)
        .map_err(|_| SxtError::CertVerifyFailed)?;
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
                // RFC-0.35-001 R6 (closes E-070): `w0` is carried for real
                // now (`send_tag`/`send_sxt` use `sys_ipc_send_words`), so
                // this decode is no longer dead — encodes kind (high 8
                // bits) | server_name_ptr (low 24 bits), exactly as always
                // documented. `upgraded` is this channel's only wired
                // caller (RFC-0.35-001 R5; diagnosticsd's and attestd's
                // slots remain uninstalled, RFC-0.35-002's decision), and it
                // always asks for `UpdateMetadata` — the fallback arm is
                // reachable only once another caller is wired up.
                let kind_raw = (w0 >> 24) as u8;
                let kind = match kind_raw {
                    0x01 => ChannelKind::UpdateMetadata,
                    0x02 => ChannelKind::Diagnostics,
                    0x03 => ChannelKind::Attestation,
                    _ => ChannelKind::UpdateMetadata,
                };
                let mut server_name = [0u8; 64];
                // Default SNI for update channel.
                let sni = b"update.fjell.example";
                server_name[..sni.len()].copy_from_slice(sni);

                match channels.open(kind, server_name) {
                    Ok(channel_id) => match perform_tls_handshake(channel_id, &server_name) {
                        Ok(epoch) => {
                            if let Some(ch) = channels.find_mut(channel_id) {
                                ch.state = ChannelState::Established;
                                ch.anchor_epoch = epoch;
                            }
                            // The channel id travels in the reply now — the
                            // fact `UpdateMetadataFetch`/`Close` need to
                            // address the right channel rather than `0`.
                            send_tag(CAP_UPGRAD_EP, SxtTag::Opened as u16, channel_id as usize);
                            sys_debug_writeln("secure-transportd: channel opened");
                        }
                        Err(_) => {
                            channels.close(channel_id);
                            send_tag(CAP_UPGRAD_EP, SxtTag::Faulted as u16, 0);
                            sys_debug_writeln("secure-transportd: handshake failed");
                        }
                    },
                    Err(_) => {
                        send_tag(CAP_UPGRAD_EP, SxtTag::Faulted as u16, 0);
                        sys_debug_writeln("secure-transportd: channel table full");
                    }
                }
            }
            Some(SxtTag::UpdateMetadataFetch) => {
                // RFC-0.35-001 R6: `w0` is the channel id the `Opened` reply
                // carried — real now, not the always-`0` E-034 left it as.
                let channel_id = w0 as u32;
                if let Some(ch) = channels.find_mut(channel_id) {
                    if ch.state == ChannelState::Established {
                        // Stub: in alpha.2 this sends an HTTP/1.1 GET over the TLS record.
                        send_tag(
                            CAP_UPGRAD_EP,
                            SxtTag::UpdateMetadataReply as u16,
                            channel_id as usize,
                        );
                    } else {
                        send_tag(CAP_UPGRAD_EP, SxtTag::Faulted as u16, 0);
                    }
                } else {
                    send_tag(CAP_UPGRAD_EP, SxtTag::Faulted as u16, 0);
                }
            }
            Some(SxtTag::Close) => {
                let channel_id = w0 as u32;
                channels.close(channel_id);
                send_tag(CAP_UPGRAD_EP, SxtTag::Closed as u16, 0);
            }
            _ => {}
        }
    }
}
