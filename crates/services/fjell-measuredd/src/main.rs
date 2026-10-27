//! measuredd — Measurement chain service for Fjell OS.
//!
//! v0.3.0-alpha.1: calls `set_measurement()` on the trust provider after
//! every chain append, keeping the provider's chain-head view current for
//! attestation (RFC v0.3-001 §5.3).
#![no_std]
#![no_main]
mod rt;

use fjell_measure_format::{
    Digest32, MeasurementError, MeasurementEvent, MeasurementHead, MeasurementKind,
    MeasurementSource, MeasurementSubject, decode_append_word, require_init,
};
use fjell_service_api::measuredd as proto;
use fjell_syscall::{sys_debug_write, sys_debug_writeln, sys_exit};
use fjell_trust_provider::descriptor::TrustProviderDescriptor;
use fjell_trust_provider::development::DevelopmentTrustProvider;
use fjell_trust_provider::ids::TrustProviderId;
use fjell_trust_provider::profile::{
    TrustProfile, TrustProviderCapabilities, TrustProviderKind, TrustProviderState,
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

/// One received request: the tag, four payload words, and the **kernel-attested
/// image id of the sender** (RFC-0.39-001). The image id is the stable half of
/// the attested identity word; nothing in the payload can change it.
struct Request {
    tag: usize,
    w: [usize; 4],
    sender_image: u16,
}

fn recv_call() -> Request {
    // RFC-0.28-002: was a hand-rolled `IpcRecv` asm block; `sys_ipc_recv_msg`
    // is a correct superset.
    match fjell_syscall::sys_ipc_recv_msg(EP_SLOT) {
        Ok((t, w0, w1, w2, w3, sender)) => Request {
            tag: t,
            w: [w0, w1, w2, w3],
            sender_image: fjell_syscall::ipc_sender_image_id(sender),
        },
        // A failed receive has no sender to attest. `u16::MAX` is no service's
        // image id, and the tag is 0, which matches no operation below.
        Err(_) => Request {
            tag: 0,
            w: [0; 4],
            sender_image: u16::MAX,
        },
    }
}

/// RFC-0.28-002 (E-032 audit, kept — escalated, not deleted): `IpcReply`
/// with a word payload has no wrapper in `fjell-syscall` —
/// `sys_ipc_reply` only carries the tag; the kernel's `sys_ipc_reply`
/// (`crates/fjell-kernel/src/cap/syscall.rs`) unconditionally copies the
/// replier's `a2..a5` into the caller's frame, so a real word payload can
/// only be sent by declaring those registers here directly. See the
/// governing RFC's answer document (§ "Finding 1") for why this is kept
/// rather than deleted, and for the allowlisted-guard rule this site must
/// satisfy.
///
/// RFC-0.38-001 R4 (closes E-085): widened from three words to **four**. The
/// kernel copies four, so a reply is exactly 32 bytes — one whole digest — and
/// the old helper, which declared `a2..a4` and left `a5` to whatever the
/// register held, could not return one. Whatever a caller does not need it
/// passes as zero: nothing is left to chance in a register the kernel reads.
fn reply(tag: usize, w0: usize, w1: usize, w2: usize, w3: usize) {
    // SAFETY: category=kernel-global-mutable measurement chain ring is exclusively owned by this service via the MeasurementDrain cap.
    unsafe {
        core::arch::asm!(
            "li a7, 23", "ecall",
            inlateout("a0") 0usize => _, in("a1") tag,
            in("a2") w0, in("a3") w1, in("a4") w2, in("a5") w3,
            lateout("a7") _, options(nostack)
        );
    }
}

/// Print one hex line: `<label><64 hex digits>`. R6's recorded value.
fn print_digest_line(label: &str, d: &Digest32) {
    let mut hex = [0u8; 64];
    sys_debug_write(label);
    sys_debug_writeln(d.to_hex(&mut hex));
}

/// Print `n` in decimal (no allocation; `sys_debug_write` takes a `&str`).
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
    // The slice is ASCII digits by construction.
    if let Ok(s) = core::str::from_utf8(&buf[i..]) {
        sys_debug_write(s);
    }
}

/// RFC-0.39-001 R5: print what the chain RECORDED for `seq`, read back out of the
/// stored event -- not what the caller sent. A tier compares these lines with
/// the provenance each event ought to carry (the numbers are the wire values of
/// `MeasurementKind` / `MeasurementSource` / `MeasurementSubject`).
fn print_event_line(chain: &Chain, seq: u64) {
    if let Some(ev) = chain.event(seq) {
        sys_debug_write("measuredd: event seq=");
        print_dec(ev.seq);
        sys_debug_write(" kind=");
        print_dec(ev.kind.as_u8() as u64);
        sys_debug_write(" source=");
        print_dec(ev.source.as_u8() as u64);
        sys_debug_write(" subject=");
        print_dec(ev.subject.as_u8() as u64);
        sys_debug_writeln("");
    }
}

// ── Measurement chain ────────────────────────────────────────────────────────

const MAX_EVENTS: usize = 64;

struct Chain {
    events: [Option<MeasurementEvent>; MAX_EVENTS],
    head: MeasurementHead,
    next_seq: u64,
}

impl Chain {
    const fn new() -> Self {
        Self {
            events: [None; MAX_EVENTS],
            head: MeasurementHead::EMPTY,
            next_seq: 1,
        }
    }

    fn append(
        &mut self,
        kind: MeasurementKind,
        source: MeasurementSource,
        subject: MeasurementSubject,
        sd: Digest32,
        meta: Option<Digest32>,
    ) -> Result<(u64, Digest32), MeasurementError> {
        let seq = self.next_seq;
        let prev = self.head.chain_digest;
        let ev = MeasurementEvent::new(seq, 0, kind, source, subject, sd, meta, prev);
        self.events[((seq - 1) as usize) % MAX_EVENTS] = Some(ev);
        self.head = MeasurementHead {
            latest_seq: seq,
            chain_digest: ev.chain_digest,
            dropped: self.head.dropped,
            last_event_kind: kind,
        };
        self.next_seq = seq + 1;
        Ok((seq, ev.chain_digest))
    }

    /// The event recorded at `seq`, read back out of the chain — what is
    /// *stored*, not what the caller sent. `None` once the ring has overwritten
    /// it.
    fn event(&self, seq: u64) -> Option<&MeasurementEvent> {
        self.events[((seq - 1) as usize) % MAX_EVENTS]
            .as_ref()
            .filter(|e| e.seq == seq)
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
        let req = recv_call();
        let [w0, w1, w2, w3] = req.w;
        let tag = req.tag & 0xFFFF;
        match tag {
            // The three verification-result events. Their subject digests are
            // placeholders `init` sends as literals, and only 16 of the 32 bytes
            // travel (see `proto::APPEND_EVENT`): RFC-0.38-001 R7 leaves them
            // disclosed as such rather than pretending they measure anything.
            //
            // RFC-0.39-001 (E-096): the SOURCE is not read from the request. It
            // is derived from the kernel-attested sender, and a request that
            // tries to declare one, names an unknown kind or subject, or comes
            // from an appender with no source is refused with the reason.
            proto::APPEND_EVENT => match decode_append_word(req.sender_image, w0) {
                Ok((kind, source, subject)) => {
                    let mut sd = [0u8; 32];
                    sd[0..8].copy_from_slice(&w1.to_le_bytes());
                    sd[8..16].copy_from_slice(&w2.to_le_bytes());
                    match chain.append(kind, source, subject, Digest32(sd), None) {
                        Ok((seq, _)) => {
                            // v0.3: keep provider chain-head view current.
                            provider.set_measurement(chain.head);
                            print_event_line(&chain, seq);
                            reply(proto::APPEND_OK, seq as usize, 0, 0, 0);
                        }
                        Err(_) => reply(proto::ERR, MeasurementError::Internal as usize, 0, 0, 0),
                    }
                }
                Err(e) => reply(proto::ERR, e as usize, 0, 0, 0),
            },
            // RFC-0.38-001 R2: the boot-time measurement of the service images
            // the kernel embeds. All 32 bytes arrive (w0..w3), and the
            // classification is fixed here so a caller cannot relabel it.
            //
            // RFC-0.39-001: and only `init` may send it. `source: Kernel` means
            // the kernel produced the digest; without this check it would be a
            // label any holder of the capability could obtain.
            proto::APPEND_IMAGE_SET if require_init(req.sender_image).is_err() => reply(
                proto::ERR,
                MeasurementError::PermissionDenied as usize,
                0,
                0,
                0,
            ),
            proto::APPEND_IMAGE_SET => {
                let sd = Digest32(proto::words_to_digest([w0, w1, w2, w3]));
                match chain.append(
                    MeasurementKind::BootEvidenceImported,
                    MeasurementSource::Kernel,
                    MeasurementSubject::BootEvidence,
                    sd,
                    None,
                ) {
                    Ok((seq, _)) => {
                        provider.set_measurement(chain.head);
                        // R6: print what the chain RECORDED, read back out of the
                        // stored event. A tier compares this line with a digest it
                        // recomputes from the images on the host.
                        if let Some(ev) = chain.event(seq) {
                            print_digest_line("measuredd: image-set digest ", &ev.subject_digest);
                        }
                        print_event_line(&chain, seq);
                        reply(proto::APPEND_OK, seq as usize, 0, 0, 0);
                    }
                    Err(_) => reply(proto::ERR, MeasurementError::Internal as usize, 0, 0, 0),
                }
            }
            // RFC-0.38-001 R4 (closes E-085): the chain digest, whole.
            proto::GET_HEAD => {
                let w = proto::digest_to_words(&chain.head.chain_digest.0);
                reply(proto::HEAD_REPLY, w[0], w[1], w[2], w[3]);
            }
            // ...and the head's bookkeeping, which no longer fits beside it.
            proto::GET_HEAD_STATS => {
                let h = chain.head;
                reply(
                    proto::HEAD_STATS_REPLY,
                    h.latest_seq as usize,
                    h.dropped as usize,
                    h.last_event_kind.as_u8() as usize,
                    0,
                );
            }
            // E-087: still a stub that sends no content; only its arity changed.
            proto::EXPORT_LOG => {
                reply(proto::EXPORT_CHUNK, chain.head.latest_seq as usize, 0, 0, 0);
                reply(proto::EXPORT_DONE, 0, 0, 0, 0);
            }
            _ => reply(proto::ERR, MeasurementError::Internal as usize, 0, 0, 0),
        }
    }
}
