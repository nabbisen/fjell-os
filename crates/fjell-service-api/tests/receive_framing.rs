//! RFC-0.32-002 D3 — the five cases the receive loop used to absorb.
//!
//! Each test drives the same sequence the two service loops drive
//! (`BEGIN` → `CHUNK`… → `COMMIT` → decode), so what is under test is the
//! receive path, not the framing type in isolation. The table in the
//! handoff's §4 is one test per row, in order.

use fjell_semantic_format::{
    ActionId, ActionKind, ActionSpec, ConfirmationPolicy, FixedVec, IntentKind, IntentNode, NodeId,
    Reversibility, SemanticEnvelope, SemanticPayload, Severity, TextToken, wire,
};
use fjell_service_api::chunked::{CHUNK_BYTES, FrameError, Reassembler};

const BUF: usize = wire::MAX_WIRE_BYTES.div_ceil(CHUNK_BYTES) * CHUNK_BYTES;

/// What the loop replies. `Err` is `PUBLISH_ERR` / `ERR` on the wire.
#[derive(Debug, PartialEq, Eq)]
enum Reply {
    Ok,
    Err,
}

/// A receive loop, with the same ordering the services use: framing first,
/// then decode, and the buffer reset after every COMMIT.
struct Receiver {
    frames: Reassembler<BUF>,
    /// The last envelope this receiver accepted — what a real service would
    /// have rendered or forwarded.
    accepted: Option<SemanticEnvelope>,
    accepted_count: usize,
}

impl Receiver {
    fn new() -> Self {
        Receiver {
            frames: Reassembler::new(),
            accepted: None,
            accepted_count: 0,
        }
    }

    fn begin(&mut self, declared: usize) -> Reply {
        match self.frames.begin(declared) {
            Ok(()) => Reply::Ok,
            Err(_) => Reply::Err,
        }
    }

    fn chunk(&mut self, w: [usize; 4]) -> Reply {
        match self.frames.chunk(w[0], w[1], w[2], w[3]) {
            Ok(()) => Reply::Ok,
            Err(_) => {
                self.frames.reset();
                Reply::Err
            }
        }
    }

    fn commit(&mut self) -> Reply {
        let decoded = match self.frames.commit() {
            Ok(bytes) => wire::decode_exact(bytes).ok(),
            Err(_) => None,
        };
        self.frames.reset();
        match decoded {
            Some(env) => {
                self.accepted = Some(env);
                self.accepted_count += 1;
                Reply::Ok
            }
            None => Reply::Err,
        }
    }

    /// The whole happy path, as `chunked::send` drives it.
    fn deliver(&mut self, bytes: &[u8]) -> Reply {
        assert_eq!(self.begin(bytes.len()), Reply::Ok);
        for w in chunks_of(bytes) {
            assert_eq!(self.chunk(w), Reply::Ok);
        }
        self.commit()
    }
}

/// Split `bytes` into 32-byte chunks of four words, zero-padding the last —
/// byte for byte what `chunked::send` puts on the wire.
fn chunks_of(bytes: &[u8]) -> Vec<[usize; 4]> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let n = (bytes.len() - i).min(CHUNK_BYTES);
        let mut c = [0u8; CHUNK_BYTES];
        c[..n].copy_from_slice(&bytes[i..i + n]);
        out.push([
            usize::from_le_bytes(c[0..8].try_into().unwrap()),
            usize::from_le_bytes(c[8..16].try_into().unwrap()),
            usize::from_le_bytes(c[16..24].try_into().unwrap()),
            usize::from_le_bytes(c[24..32].try_into().unwrap()),
        ]);
        i += n;
    }
    out
}

fn sample_intent(sequence: u64) -> SemanticEnvelope {
    SemanticEnvelope::new_intent(
        NodeId {
            producer_index: 6,
            local_sequence: 1,
        },
        sequence,
        IntentNode {
            kind: IntentKind::ActionRequest,
            title: TextToken::new("a title"),
            description: TextToken::new("a description"),
            severity: Severity::Normal,
            actions: FixedVec::new(),
            consequences: FixedVec::new(),
            expires_at_tick: None,
        },
    )
}

/// An intent wide enough to span several chunks, so "one chunk short" is a
/// distinct case from "no chunks at all".
fn multi_chunk_intent() -> SemanticEnvelope {
    let mut env = sample_intent(1);
    if let SemanticPayload::Intent(n) = &mut env.payload {
        for i in 0..4 {
            let _ = n.actions.push(ActionSpec {
                action_id: ActionId(i),
                label: TextToken::new("an action label"),
                kind: ActionKind::Confirm,
                required_capability: None,
                reversibility: Reversibility::Reversible,
                confirmation: ConfirmationPolicy::Required,
            });
        }
    }
    env
}

fn encoded(env: &SemanticEnvelope) -> Vec<u8> {
    let mut out = [0u8; wire::MAX_WIRE_BYTES];
    let n = wire::encode(env, &mut out).expect("the sample envelope encodes");
    out[..n].to_vec()
}

/// The positive control: without this, every refusal below could be a
/// receiver that refuses everything.
#[test]
fn a_well_framed_message_is_accepted() {
    let mut r = Receiver::new();
    assert_eq!(r.deliver(&encoded(&sample_intent(1))), Reply::Ok);
    assert_eq!(r.accepted_count, 1);
    assert_eq!(r.accepted.unwrap().sequence, 1);
}

/// Row 1: `BEGIN` then `COMMIT` with no chunks. Today this reinterprets the
/// previous message's bytes; the assertion that matters is not just the
/// refusal but that the previous message is not delivered a second time.
#[test]
fn begin_then_commit_with_no_chunks_is_refused() {
    let mut r = Receiver::new();
    assert_eq!(r.deliver(&encoded(&sample_intent(1))), Reply::Ok);
    assert_eq!(r.accepted_count, 1);

    let declared = encoded(&sample_intent(2)).len();
    assert_eq!(r.begin(declared), Reply::Ok);
    assert_eq!(r.commit(), Reply::Err);

    assert_eq!(r.accepted_count, 1, "the previous message was re-delivered");
    assert_eq!(r.accepted.unwrap().sequence, 1);
}

/// Row 2: fewer chunk bytes than `BEGIN` declared. Today the shortfall is the
/// previous message's tail.
#[test]
fn fewer_bytes_than_declared_is_refused() {
    let mut r = Receiver::new();
    let bytes = encoded(&multi_chunk_intent());
    assert!(bytes.len() > CHUNK_BYTES * 2, "need a multi-chunk message");

    assert_eq!(r.begin(bytes.len()), Reply::Ok);
    let all = chunks_of(&bytes);
    for w in &all[..all.len() - 1] {
        assert_eq!(r.chunk(*w), Reply::Ok);
    }
    assert_eq!(r.commit(), Reply::Err);
    assert_eq!(r.accepted_count, 0);
}

/// Row 3: more chunks than declared. Today `write_chunk` drops them silently.
#[test]
fn more_chunks_than_declared_is_refused() {
    let mut r = Receiver::new();
    let bytes = encoded(&sample_intent(1));

    assert_eq!(r.begin(bytes.len()), Reply::Ok);
    for w in chunks_of(&bytes) {
        assert_eq!(r.chunk(w), Reply::Ok);
    }
    // One chunk past the declared length.
    assert_eq!(r.chunk([0; 4]), Reply::Err);
    // The refusal reset the message, so the COMMIT has no BEGIN behind it.
    assert_eq!(r.commit(), Reply::Err);
    assert_eq!(r.accepted_count, 0);
}

/// Row 4: `COMMIT` with no `BEGIN`. Today it reinterprets whatever is in the
/// buffer — after a valid message, that is the valid message, delivered twice.
#[test]
fn commit_with_no_begin_is_refused() {
    let mut r = Receiver::new();
    assert_eq!(r.commit(), Reply::Err);

    assert_eq!(r.deliver(&encoded(&sample_intent(1))), Reply::Ok);
    assert_eq!(r.commit(), Reply::Err, "a bare COMMIT replayed the message");
    assert_eq!(r.accepted_count, 1);
}

/// Row 5: an unknown discriminant in the bytes. Today this is undefined
/// behaviour — Miri names it at `.correlation_id.<enum-tag>`. Here it is a
/// decode error, and the framing above it was perfectly well formed.
#[test]
fn an_unknown_discriminant_is_a_decode_error() {
    let mut bytes = encoded(&sample_intent(1));
    bytes[8] = 0xFF; // the stream tag

    let mut r = Receiver::new();
    assert_eq!(r.begin(bytes.len()), Reply::Ok);
    for w in chunks_of(&bytes) {
        assert_eq!(r.chunk(w), Reply::Ok);
    }
    assert_eq!(r.commit(), Reply::Err);
    assert_eq!(r.accepted_count, 0);

    // Named, rather than inferred from the refusal.
    assert_eq!(
        wire::decode_exact(&bytes).unwrap_err(),
        wire::WireError::BadTag
    );
}

/// The stale buffer that started this: a zero-filled buffer decoded into a
/// fabricated envelope under the old path, silently and without Miri
/// objecting. It is now refused twice over — the framing never declared it,
/// and the bytes do not decode.
#[test]
fn a_zero_filled_buffer_is_refused() {
    let zeros = [0u8; 256];
    let mut r = Receiver::new();
    assert_eq!(r.begin(zeros.len()), Reply::Ok);
    for w in chunks_of(&zeros) {
        assert_eq!(r.chunk(w), Reply::Ok);
    }
    assert_eq!(r.commit(), Reply::Err);
    assert_eq!(
        wire::decode_exact(&zeros).unwrap_err(),
        wire::WireError::BadMagic
    );
}

/// A length no receiver could hold is refused at `BEGIN`, before any byte of
/// it arrives.
#[test]
fn a_declared_length_larger_than_the_buffer_is_refused_at_begin() {
    let mut r = Receiver::new();
    assert_eq!(r.begin(BUF + 1), Reply::Err);
    assert_eq!(r.chunk([0; 4]), Reply::Err);
    assert_eq!(r.commit(), Reply::Err);
}

/// The error values are distinguishable, so a service could reply differently
/// per case if it ever needed to.
#[test]
fn the_refusals_are_named() {
    let mut f = Reassembler::<BUF>::new();
    assert_eq!(f.commit().unwrap_err(), FrameError::NoBegin);
    assert_eq!(f.chunk(0, 0, 0, 0).unwrap_err(), FrameError::NoBegin);
    assert_eq!(f.begin(BUF + 1).unwrap_err(), FrameError::DeclaredTooLarge);

    f.begin(64).unwrap();
    assert_eq!(f.commit().unwrap_err(), FrameError::ShortMessage);

    f.begin(64).unwrap();
    f.chunk(0, 0, 0, 0).unwrap();
    f.chunk(0, 0, 0, 0).unwrap();
    assert_eq!(f.chunk(0, 0, 0, 0).unwrap_err(), FrameError::TooManyChunks);
}
