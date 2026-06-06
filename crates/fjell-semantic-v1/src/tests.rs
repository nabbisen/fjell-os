//! Host unit tests for `fjell-semantic-v1` (RFC v0.5-004 §7.2 schema tests).
//!
//! For every catalog entry: encode → decode → field values roundtrip.
//! Additional negative tests for truncation, bad magic, trailing bytes.

use crate::catalog::{CATALOG_V1, lookup_tag, catalog_len};
use crate::codec::{encode, decode, FieldValue, SemanticError, MAX_ENVELOPE_BYTES};
use crate::schema::FieldKind;
use crate::version::{CATALOG_V1_VERSION, CatalogVersion};

// ── Catalog integrity ─────────────────────────────────────────────────────────

#[test]
fn catalog_has_expected_entry_count() {
    // 6 update + 2 attest + 3 security + 4 net + 2 recovery + 1 platform + 2 health = 20
    assert_eq!(catalog_len(), 20);
}

#[test]
fn catalog_tags_are_unique() {
    for i in 0..CATALOG_V1.len() {
        for j in 0..CATALOG_V1.len() {
            if i != j {
                assert_ne!(CATALOG_V1[i].tag, CATALOG_V1[j].tag,
                    "duplicate tag {:#06x}", CATALOG_V1[i].tag);
            }
        }
    }
}

#[test]
fn catalog_tags_sorted_ascending() {
    for i in 1..CATALOG_V1.len() {
        assert!(CATALOG_V1[i].tag > CATALOG_V1[i-1].tag,
            "catalog not sorted at index {i}");
    }
}

#[test]
fn catalog_reserved_ranges_not_used() {
    for e in CATALOG_V1 {
        assert!(e.tag < 0x0200 || e.tag >= 0x0400,
            "tag {:#06x} falls in reserved FLEET/SDK range", e.tag);
    }
}

#[test]
fn catalog_version_is_v1_0() {
    assert_eq!(CATALOG_V1_VERSION, CatalogVersion::V1_0);
}

#[test]
fn lookup_unknown_tag_returns_none() {
    assert!(lookup_tag(0x0000).is_none());
    assert!(lookup_tag(0x0200).is_none());  // FLEET reserved
    assert!(lookup_tag(0xFFFF).is_none());
}

#[test]
fn lookup_known_tag_returns_entry() {
    let e = lookup_tag(0x0100).unwrap();
    assert_eq!(e.name, "UPDATE.STAGING_STARTED");
    assert_eq!(e.schema.fields.len(), 3);
}

// ── Codec round-trips ─────────────────────────────────────────────────────────

/// Build a synthetic list of `FieldValue`s matching the schema of `entry`,
/// using simple incrementing values.
fn make_test_fields(entry: &crate::catalog::IntentEntry) -> [FieldValue; 6] {
    let mut fv = [FieldValue::Absent; 6];
    for (i, fd) in entry.schema.fields.iter().enumerate() {
        fv[i] = match fd.kind {
            FieldKind::U8      => FieldValue::U8((i as u8).wrapping_add(1)),
            FieldKind::U16     => FieldValue::U16(i as u16 + 0x0100),
            FieldKind::U32     => FieldValue::U32(i as u32 + 0xDEAD_0000),
            FieldKind::U64     => FieldValue::U64(i as u64 + 0xCAFE_BABE_0000_0000),
            FieldKind::Bytes16 => { let mut b = [0u8;16]; b[0] = i as u8; FieldValue::Bytes16(b) }
            FieldKind::Bytes32 => { let mut b = [0u8;32]; b[0] = i as u8; FieldValue::Bytes32(b) }
        };
    }
    fv
}

#[test]
fn all_catalog_entries_round_trip() {
    let mut buf = [0u8; MAX_ENVELOPE_BYTES];
    for entry in CATALOG_V1 {
        let fv = make_test_fields(entry);
        let n = encode(entry.tag, 12345, &fv[..entry.schema.fields.len()], &mut buf)
            .unwrap_or_else(|e| panic!("encode failed for {}: {:?}", entry.name, e));
        let decoded = decode(&buf[..n])
            .unwrap_or_else(|e| panic!("decode failed for {}: {:?}", entry.name, e));
        assert_eq!(decoded.tag, entry.tag);
        assert_eq!(decoded.created_tick, 12345);
        for (i, _fd) in entry.schema.fields.iter().enumerate() {
            assert_ne!(decoded.fields[i], FieldValue::Absent,
                "field {i} of {} is absent after roundtrip", entry.name);
            assert_eq!(decoded.fields[i], fv[i],
                "field {i} mismatch for {}", entry.name);
        }
    }
}

// ── Negative tests ────────────────────────────────────────────────────────────

#[test]
fn decode_empty_buffer_returns_truncated_magic() {
    assert_eq!(decode(&[]), Err(SemanticError::TruncatedMagic));
}

#[test]
fn decode_bad_magic_returns_error() {
    let mut buf = [0u8; 20];
    buf[..7].copy_from_slice(b"BADBAD!");
    assert_eq!(decode(&buf), Err(SemanticError::BadMagic));
}

#[test]
fn decode_truncated_body_returns_error() {
    let mut buf = [0u8; MAX_ENVELOPE_BYTES];
    let entry = lookup_tag(0x0100).unwrap();
    let fv = make_test_fields(entry);
    let n = encode(entry.tag, 0, &fv[..entry.schema.fields.len()], &mut buf).unwrap();
    // Truncate by 3 bytes.
    assert_eq!(decode(&buf[..n-3]), Err(SemanticError::TruncatedBody));
}

#[test]
fn decode_missing_sentinel_returns_error() {
    let mut buf = [0u8; MAX_ENVELOPE_BYTES];
    let entry = lookup_tag(0x0100).unwrap();
    let fv = make_test_fields(entry);
    let n = encode(entry.tag, 0, &fv[..entry.schema.fields.len()], &mut buf).unwrap();
    // Corrupt the sentinel byte.
    buf[n-1] = 0xAA;
    assert_eq!(decode(&buf[..n]), Err(SemanticError::MissingSentinel));
}

#[test]
fn decode_trailing_bytes_returns_error() {
    let mut buf = [0u8; MAX_ENVELOPE_BYTES + 4];
    let entry = lookup_tag(0x0100).unwrap();
    let fv = make_test_fields(entry);
    let n = encode(entry.tag, 0, &fv[..entry.schema.fields.len()], &mut buf).unwrap();
    // Pass extra bytes after valid envelope.
    buf[n] = 0x00; // one trailing byte
    assert_eq!(decode(&buf[..n+1]), Err(SemanticError::TrailingBytes));
}

#[test]
fn encode_unknown_tag_returns_error() {
    let mut buf = [0u8; MAX_ENVELOPE_BYTES];
    assert_eq!(encode(0x9999, 0, &[], &mut buf), Err(SemanticError::UnknownTag));
}

#[test]
fn encode_required_field_absent_returns_error() {
    let mut buf = [0u8; MAX_ENVELOPE_BYTES];
    // Tag 0x0100 has 3 required fields; pass empty slice.
    assert_eq!(
        encode(0x0100, 0, &[], &mut buf),
        Err(SemanticError::RequiredFieldMissing)
    );
}

#[test]
fn encode_buffer_too_small_returns_error() {
    let mut tiny = [0u8; 5];
    let entry = lookup_tag(0x0100).unwrap();
    let fv = make_test_fields(entry);
    assert_eq!(
        encode(entry.tag, 0, &fv[..entry.schema.fields.len()], &mut tiny),
        Err(SemanticError::BufferTooSmall)
    );
}
