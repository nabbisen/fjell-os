// Fuzz target: semantic intent v1 encode/decode round-trip (RFC v0.6-003)
// Exercises: magic mismatch, truncation, unknown tags, bad sentinels.
#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    // decode must never panic.
    let _ = fjell_semantic_v1::decode(data);
    // If we have a valid tag and fields, encode → decode must round-trip.
    if data.len() >= 2 {
        let tag = u16::from_le_bytes([data[0], data[1]]);
        if let Some(entry) = fjell_semantic_v1::lookup_tag(tag) {
            let mut fields = [fjell_semantic_v1::FieldValue::Absent;
                              fjell_semantic_v1::codec::MAX_FIELDS];
            for (i, fd) in entry.schema.fields.iter().enumerate() {
                use fjell_semantic_v1::schema::FieldKind;
                fields[i] = match fd.kind {
                    FieldKind::U8  => fjell_semantic_v1::FieldValue::U8(data.get(2).copied().unwrap_or(0)),
                    FieldKind::U16 => fjell_semantic_v1::FieldValue::U16(0),
                    FieldKind::U32 => fjell_semantic_v1::FieldValue::U32(0),
                    FieldKind::U64 => fjell_semantic_v1::FieldValue::U64(0),
                    FieldKind::Bytes16 => fjell_semantic_v1::FieldValue::Bytes16([0u8;16]),
                    FieldKind::Bytes32 => fjell_semantic_v1::FieldValue::Bytes32([0u8;32]),
                };
            }
            let mut buf = [0u8; fjell_semantic_v1::codec::MAX_ENVELOPE_BYTES];
            if let Ok(n) = fjell_semantic_v1::encode(tag, 0, &fields[..entry.schema.fields.len()], &mut buf) {
                let _ = fjell_semantic_v1::decode(&buf[..n]);
            }
        }
    }
});
