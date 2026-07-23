//! Host unit tests for `fjell-summary-format` (RFC v0.7-003 §11).

use crate::measurement::{
    MeasurementSummary, MeasurementKindCount,
    MSUMMARY_SCHEMA_VERSION, MAX_KIND_COUNTS,
};
use crate::release::{
    ReleaseSummary, ChannelSummary, AdvanceSource,
    RSUMMARY_SCHEMA_VERSION,
};
use crate::digest::{measurement_summary_digest, release_summary_digest};
use fjell_measure_format::Digest32;

fn dummy_msummary() -> MeasurementSummary {
    MeasurementSummary::new(
        [0x01u8; 16],
        50_000,
        127,
        Digest32([0xAAu8; 32]),
        Digest32([0xBBu8; 32]),
    )
}

fn dummy_rsummary() -> ReleaseSummary {
    ReleaseSummary::new([0x02u8; 16], 60_000)
}

// ── Schema constants ──────────────────────────────────────────────────────────

#[test]
fn msummary_schema_version_is_one() {
    assert_eq!(MSUMMARY_SCHEMA_VERSION, 1);
}

#[test]
fn rsummary_schema_version_is_one() {
    assert_eq!(RSUMMARY_SCHEMA_VERSION, 1);
}

// ── MeasurementSummary ────────────────────────────────────────────────────────

#[test]
fn add_kind_count_increments() {
    let mut s = dummy_msummary();
    s.add_kind_count(0x01, 5).unwrap();
    s.add_kind_count(0x02, 3).unwrap();
    assert_eq!(s.kind_count, 2);
    assert_eq!(s.kind_counts[0], MeasurementKindCount { kind: 0x01, count: 5 });
}

#[test]
fn add_kind_count_rejects_at_capacity() {
    let mut s = dummy_msummary();
    for i in 0..MAX_KIND_COUNTS {
        s.add_kind_count(i as u8, 1).unwrap();
    }
    assert_eq!(s.add_kind_count(0xFF, 1), Err(crate::measurement::SummaryError::CapacityExhausted));
}

#[test]
fn measurement_digest_is_nonzero() {
    let s = dummy_msummary();
    assert_ne!(measurement_summary_digest(&s).0, [0u8; 32]);
}

#[test]
fn measurement_digest_is_deterministic() {
    let s = dummy_msummary();
    assert_eq!(measurement_summary_digest(&s).0, measurement_summary_digest(&s).0);
}

#[test]
fn measurement_digest_sensitive_to_head_seq() {
    let s1 = dummy_msummary();
    let mut s2 = dummy_msummary();
    s2.head_seq = 999;
    assert_ne!(measurement_summary_digest(&s1).0, measurement_summary_digest(&s2).0);
}

// ── ReleaseSummary ────────────────────────────────────────────────────────────

#[test]
fn add_channel_increments() {
    let mut s = dummy_rsummary();
    s.add_channel(ChannelSummary {
        channel_id: *b"chan-001",
        current_counter: 42,
        min_counter: 10,
        active_anchor_epoch: 3,
        last_confirm_tick: 1000,
        last_advance_source: AdvanceSource::LocalInstall,
    }).unwrap();
    assert_eq!(s.channel_count, 1);
}

#[test]
fn release_digest_is_nonzero() {
    let s = dummy_rsummary();
    assert_ne!(release_summary_digest(&s).0, [0u8; 32]);
}

#[test]
fn release_digest_sensitive_to_channel_data() {
    let mut s1 = dummy_rsummary();
    let mut s2 = dummy_rsummary();
    let ch = ChannelSummary {
        channel_id: *b"chan-001",
        current_counter: 42,
        min_counter: 10,
        active_anchor_epoch: 3,
        last_confirm_tick: 1000,
        last_advance_source: AdvanceSource::LocalInstall,
    };
    s1.add_channel(ch).unwrap();
    // s2 has different counter
    let mut ch2 = ch;
    ch2.current_counter = 99;
    s2.add_channel(ch2).unwrap();
    assert_ne!(release_summary_digest(&s1).0, release_summary_digest(&s2).0);
}

#[test]
fn advance_source_roundtrip() {
    for (byte, expected) in [
        (0u8, AdvanceSource::Unknown),
        (1,   AdvanceSource::LocalInstall),
        (2,   AdvanceSource::SnapshotSync),
        (3,   AdvanceSource::ManualPinned),
    ] {
        assert_eq!(AdvanceSource::from_u8(byte).unwrap() as u8, expected as u8);
    }
    assert!(AdvanceSource::from_u8(0xFF).is_none());
}

// ── RFC-v0.7.5-001: duplicate rejection ──────────────────────────────────────

#[test]
fn measurement_summary_rejects_duplicate_kind() {
    let mut s = MeasurementSummary::new([0u8; 16], 0, 0, fjell_measure_format::Digest32([0u8; 32]), fjell_measure_format::Digest32([0u8; 32]));
    s.add_kind_count(0x01, 5).unwrap();
    assert_eq!(
        s.add_kind_count(0x01, 3),
        Err(crate::measurement::SummaryError::DuplicateKind)
    );
    // Count is unchanged
    assert_eq!(s.kind_count, 1);
}

#[test]
fn release_summary_rejects_duplicate_channel() {
    let mut s = ReleaseSummary::new([0u8; 16], 0);
    let ch = ChannelSummary {
        channel_id: *b"chan0001",
        current_counter: 10,
        min_counter: 1,
        active_anchor_epoch: 1,
        last_confirm_tick: 100,
        last_advance_source: AdvanceSource::LocalInstall,
    };
    s.add_channel(ch).unwrap();
    assert_eq!(
        s.add_channel(ch),
        Err(crate::measurement::SummaryError::DuplicateChannel)
    );
    assert_eq!(s.channel_count, 1);
}
