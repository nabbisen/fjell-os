//! Golden digests: the bytes each format's encoder produced **before**
//! RFC-0.33-003 rewrote it to write through `Canon`.
//!
//! They were captured from the unmodified encoders (the commit that added this
//! file touches no format crate) and must never be regenerated to make a change
//! pass: an encoder that no longer reproduces one has changed a wire format, and
//! every digest and signature stored under the old bytes stops verifying. Change
//! a constant here only with a version bump and a note.
//!
//! Each sample gives every field a distinct value (`fjell_schema::samples`), so a
//! reordered, dropped or re-widthed field changes the digest.

use fjell_schema::samples as s;

fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

fn check(name: &str, got: &[u8], want: &str) {
    assert_eq!(
        hex(got),
        want,
        "{name}: the canonical byte stream changed — this is a wire-format change"
    );
}

#[test]
fn rollback_record() {
    check(
        "rollback_record",
        &s::rollback_record().compute_digest().0,
        "711a40e750f357b3590e5cc26e8e9c7280f00ab50aa7cb60daaa8e70979f10be",
    );
}

#[test]
fn release_metadata() {
    check(
        "release_metadata",
        &s::release_metadata().compute_digest().0,
        "26ff3ba15eb1353854f8ec6c818aef13524dc3470b6e1447f0bc1b044c7111c4",
    );
}

#[test]
fn attestation_v2() {
    check(
        "attestation_v2",
        &s::attestation_v2().canonical_digest().0,
        "92913e8021da2ff2216e4b7bcc06a507e8023b5d121a4b1a436ab823ffd48d3e",
    );
}

#[test]
fn diag_bundle() {
    // RFC-0.35-002 (closes E-068), DIAG_BUNDLE_VERSION 1 -> 2: a new field,
    // `audit_available`, is now written into the canonical byte stream
    // this digest covers (ahead of `audit_event_count`, so it distinguishes
    // "not implemented" from "queried, zero records" for a caller reading
    // the bundle). A version bump and this note, as this file's own header
    // requires before a stored digest may change.
    //
    // RFC-0.36-001 (closes E-076, E-079, E-080), DIAG_BUNDLE_VERSION 2 -> 3:
    // the one line in this project's history where a changed digest is the
    // point, not a defect — `write_canonical` now writes `provider_id` and
    // `keyring_anchor_epoch` (E-080: both were struct fields the signed
    // digest silently omitted, though RFC-v0.4-005 §6.4 lists both) and
    // three new availability flags, each adjacent to the field it qualifies
    // (`keyring_anchor_epoch_available`, `measurement_head_available`,
    // `last_attestation_available`). The sample's `measurement_head` is now
    // marked available (`true`), the "a real query succeeded" case.
    //
    // RFC-0.39-002 (R4), DIAG_BUNDLE_VERSION 3 -> 4: two fixed-width `u32`
    // fields, `measurement_event_count` and `measurement_events_kernel_digested`,
    // are written immediately after `measurement_head`, zeroed by the encoder
    // while the head is unavailable. The sample (head available, 4 events, 1
    // kernel-digested) therefore has a new digest. **It was not taken from the
    // encoder on trust:** `diag_bundle_v4_is_v3_plus_two_counts` below proves the
    // v4 stream is exactly the v3 stream with those eight bytes inserted and the
    // version word changed, by recovering the v3 golden (the previous constant,
    // `06332d60...4548`) from it. A version bump and this note, as the header
    // requires.
    check(
        "diag_bundle",
        &s::diag_bundle().bundle_digest.0,
        "eee02b623defe888ba34ecad3740137994374c5d2e3edd34ad9bc2296cf8feaa",
    );
}

/// What the 3 -> 4 bump did, shown rather than asserted: take the v4 canonical
/// stream of the sample, delete the two inserted counts (8 bytes, straight after
/// the 32-byte measurement head) and put the version word back to 3, and the
/// digest is the **v3 golden** again. Nothing else in the stream moved.
#[test]
fn diag_bundle_v4_is_v3_plus_two_counts() {
    use fjell_canon::BufSink;
    use fjell_measure_format::Digest32;
    let bundle = s::diag_bundle();
    let mut sink = BufSink::<1700>::new();
    bundle.write_canonical(&mut sink);
    let v4 = sink.bytes().to_vec();

    // 13 domain + 2 version + 8 id + 8 tick + 4 provider + 1 keyring_avail
    // + 4 keyring + 1 head_avail + 32 head = 73; the counts are bytes 73..81.
    const AT: usize = 13 + 2 + 8 + 8 + 4 + 1 + 4 + 1 + 32;
    assert_eq!(&v4[..13], b"FJELL-DIAG-V1", "the domain tag is first");
    assert_eq!(&v4[13..15], &4u16.to_le_bytes(), "the version word is 4");
    assert_eq!(
        &v4[AT..AT + 4],
        &4u32.to_le_bytes(),
        "measurement_event_count follows the head"
    );
    assert_eq!(
        &v4[AT + 4..AT + 8],
        &1u32.to_le_bytes(),
        "measurement_events_kernel_digested follows it"
    );

    let mut v3 = v4.clone();
    v3.drain(AT..AT + 8);
    v3[13..15].copy_from_slice(&3u16.to_le_bytes());
    check(
        "diag_bundle (v4 stream rewound to v3)",
        &Digest32::of(&v3).0,
        "06332d60d5f3043a01e112a3730d33191de7dbe04a54f629eae01591196d4548",
    );
}

#[test]
fn identity() {
    check(
        "identity",
        &fjell_identity_format::digest::identity_digest(&s::node_identity()).0,
        "2cc3199076e7aa529192f90a5476431d7bcf58524a3fbda989c6673e4e8b76a1",
    );
}

#[test]
fn platform() {
    check(
        "platform",
        &fjell_platform_format::digest::platform_digest(&s::platform_profile()).0,
        "6bab6ef79928e390d519940bb6b56fb23d43c669994a3686c3d37c50eb984bef",
    );
}

#[test]
fn board() {
    check(
        "board",
        &fjell_platform_format::digest::board_digest(&s::board_profile()).0,
        "9b25b44469ec9aae647b546d045ddb904044d606fcc1e7489b191842b88fabdb",
    );
}

#[test]
fn keyring_snapshot() {
    check(
        "keyring_snapshot",
        &s::keyring_snapshot().snapshot_digest.0,
        "f7a41190c5bb8552702ad9d5d081ccbb77cfb9e23116bc63955cbec547ca29b3",
    );
}

#[test]
fn measurement_summary() {
    check(
        "measurement_summary",
        &fjell_summary_format::digest::measurement_summary_digest(&s::measurement_summary()).0,
        "ecb541accf62f16f83572369b68bb91b4181abd0c2b658073f78c2a6c7b9fe50",
    );
}

#[test]
fn release_summary() {
    check(
        "release_summary",
        &fjell_summary_format::digest::release_summary_digest(&s::release_summary()).0,
        "1f50aea04d4550a8edbeddd30eb2f772087097d1a654deedfc56d8e35d2955ed",
    );
}

#[test]
fn snapshot_v1_has_no_domain_byte() {
    check(
        "snapshot_v1",
        &fjell_snapshot_format::snapshot_digest(&s::snapshot_envelope(1)).0,
        "39103194679985baf1a375c10eff4e73312f7764fb75645832dc11d63e48f505",
    );
}

#[test]
fn snapshot_v2_has_the_domain_byte() {
    check(
        "snapshot_v2",
        &fjell_snapshot_format::snapshot_digest(&s::snapshot_envelope(2)).0,
        "bcb4aea040371bad12bf59a771d04c3ac30e9ffb5a9758069608a0ac5ae1059a",
    );
}

/// The fleet roster digest, captured from the unmodified encoder on a
/// three-member roster (which fits the old 512-byte buffer, so the rewrite that
/// removed the buffer limit must not move it — E-066 changed only what happens
/// beyond eight members).
#[test]
fn fleet_roster() {
    check(
        "fleet_roster",
        &fjell_fleet_format::roster_digest(&s::fleet_roster()).0,
        "604ef51fd0d3e9a0bd0671f0554bd4956a209bb11da2cc73284621ce5b50f49e",
    );
}

#[test]
fn fleet_policy() {
    check(
        "fleet_policy",
        &fjell_fleet_format::policy_digest(&s::fleet_policy()).0,
        "fb1580eef92622b18d71ffa24fa94be931c228ce843b5f129b32be622a9c025a",
    );
}

/// The wire codec's bytes (`fjell-semantic-format::wire::encode`) for five envelopes that
/// between them reach every arm of the codec (`samples::semantic_envelopes`), captured from
/// the encoder **before** RFC-0.34-003 rewrote it to write through `Canon`. The rewrite must
/// reproduce every byte; a change here is a wire-format change to the format services
/// exchange across a trust boundary, and needs a version bump and a note.
#[test]
fn wire_envelopes() {
    let want: [(usize, usize, &str); 5] = [
        (
            0,
            183,
            "464a534501000100010101010001021817161514131211013837363534333231010544440000100055706461746520617661696c61626c6555550000170041206e65772072656c65617365206973207374616765640402010a111100000b0061636b6e6f776c656467650101070073657276696365080073746f7261676564f10000000202020a222200000500726574727903000101010333330000100064617461206d6179206265206c6f7374012827262524232221",
        ),
        (
            1,
            47,
            "464a53450100010001020102000102020000000000000000010166660000050048656c6c6f77770000000001000000",
        ),
        (
            2,
            159,
            "464a53450100010002030103000102030000000000000000020311a1000007005365727669636522a200000800416c6c2077656c6c04050090000004006661637401010301900000040066616374024847464544434241030290000004006661637403a8a8a9aaabacadae03039000000400666163740488880000050072656164790304900000040066616374056867666564636261787776757473727103",
        ),
        (
            3,
            94,
            "464a534501000100030401040001020400000000000000010900000000000000030c11b100000400446f6e6522b20000140054686520616374696f6e20636f6d706c657465640201010a0070726f78792d74657874018887868584838281",
        ),
        (
            4,
            47,
            "464a53450100010003050105000102050000000000000000030211c100000500526561647922c20000000001050000",
        ),
    ];
    let envs = s::semantic_envelopes();
    assert_eq!(envs.len(), want.len());
    for (i, len, hex_want) in want {
        let mut out = [0u8; 8192];
        let n = fjell_semantic_format::wire::encode(&envs[i], &mut out).expect("encodes");
        assert_eq!(n, len, "wire sample {i}: the length changed");
        check(&format!("wire_{i}"), &out[..n], hex_want);
    }
}

/// The samples reach every arm of the codec. The two `arm` helpers are exhaustive matches,
/// so a new variant in the model is a compile error there; this test makes sure the set
/// actually takes each arm — an arm the samples miss is a hole in the description.
#[test]
fn the_wire_samples_reach_every_arm() {
    let envs = s::semantic_envelopes();
    let mut payload = [false; s::PAYLOAD_ARMS];
    let mut fact = [false; s::FACT_ARMS];
    for e in &envs {
        payload[s::payload_arm(&e.payload)] = true;
        if let fjell_semantic_format::SemanticPayload::State(n) = &e.payload {
            for f in n.facts.iter() {
                fact[s::fact_arm(&f.value)] = true;
            }
        }
    }
    assert!(
        payload.iter().all(|&b| b),
        "a payload arm has no sample: {payload:?}"
    );
    assert!(
        fact.iter().all(|&b| b),
        "a FactValue arm has no sample: {fact:?}"
    );
}

/// The semantic intent codec's bytes, captured from `encode` before it was
/// rewritten to write through `Canon`. (No catalogue entry has an optional field,
/// so every `present` byte here is `01`.)
#[test]
fn semantic_intent_v1() {
    let (tag, bytes) = s::semantic_intent();
    assert_eq!(tag, 0x0100);
    check(
        "semantic_intent_v1",
        &bytes,
        "464a53492d563100011817161514131211011003020101110302010112030201ff",
    );
}

/// The control for the whole file: a golden that cannot fail proves nothing. The
/// same sample with one field changed must NOT reproduce its digest.
#[test]
fn a_changed_field_does_not_reproduce_its_golden() {
    let mut r = s::rollback_record();
    r.min_counter ^= 1;
    assert_ne!(
        hex(&r.compute_digest().0),
        "711a40e750f357b3590e5cc26e8e9c7280f00ab50aa7cb60daaa8e70979f10be"
    );
}
