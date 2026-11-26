//! What the chunked path costs, pinned (RFC-0.49-002 D6).
//!
//! `DmaShare` (111) was retired at 0.49, so bulk transfer between services is the chunked byte transfer, and the documents
//! (`docs/src/external-design/ipc.md` §7, `chunked`'s doc-comment) state its cost: **147 round trips** for the widest
//! envelope, **294** for one emission plus one forward hop. Those figures were wrong twice before (157/314 from the struct's
//! in-memory size; "dozens"), so they are derived here from the real constants and the real receiver, not from arithmetic
//! copied into prose. If this test moves, the documents named above move with it.

use fjell_semantic_format::wire;
use fjell_service_api::chunked::{CHUNK_BYTES, Reassembler};

const BUF: usize = wire::MAX_WIRE_BYTES.div_ceil(CHUNK_BYTES) * CHUNK_BYTES;

/// Round trips one transfer of `bytes` costs: `BEGIN`, every `CHUNK` the receiver needs, `COMMIT` -- counted by driving the
/// receiver until it accepts the commit, not by dividing.
fn round_trips_driving_the_receiver(bytes: usize) -> usize {
    let mut f = Reassembler::<BUF>::new();
    let mut trips = 1; // BEGIN
    f.begin(bytes).unwrap();
    while f.commit().is_err() {
        f.chunk(0, 0, 0, 0).unwrap();
        trips += 1;
        assert!(trips <= BUF, "the receiver never accepted the commit");
    }
    trips + 1 // COMMIT
}

#[test]
fn the_widest_envelope_is_147_round_trips_and_the_full_path_294() {
    assert_eq!(wire::MAX_WIRE_BYTES, 4624, "the bound the documents quote");
    assert_eq!(CHUNK_BYTES, 32);
    assert_eq!(wire::MAX_WIRE_BYTES.div_ceil(CHUNK_BYTES), 145, "chunks");
    let one_transfer = round_trips_driving_the_receiver(wire::MAX_WIRE_BYTES);
    assert_eq!(one_transfer, 147, "BEGIN + 145 CHUNK + COMMIT");
    assert_eq!(2 * one_transfer, 294, "one emission plus one forward hop");
}

/// The figure the documents used to carry: it came from the struct's size, which is not what is sent.
#[test]
fn the_old_157_and_314_were_the_structs_size_not_the_wire_size() {
    let struct_bytes = core::mem::size_of::<fjell_semantic_format::SemanticEnvelope>();
    assert_eq!(struct_bytes, 4936);
    assert_ne!(struct_bytes, wire::MAX_WIRE_BYTES);
    assert_eq!(struct_bytes.div_ceil(CHUNK_BYTES) + 2, 157);
}
