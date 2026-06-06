//! Semantic catalog v1 encode/decode benchmarks (RFC-v0.10-004).
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use fjell_semantic_v1::{encode, decode, FieldValue, CATALOG_V1};

fn bench_encode(c: &mut Criterion) {
    // Use the first catalog entry that has fields.
    let entry = CATALOG_V1.iter().find(|e| !e.schema.fields.is_empty()).unwrap();
    let fields: Vec<FieldValue> = entry.schema.fields.iter()
        .map(|f| match f.kind {
            fjell_semantic_v1::FieldKind::U32 => FieldValue::U32(0x01020304),
            fjell_semantic_v1::FieldKind::U16 => FieldValue::U16(0x0102),
            fjell_semantic_v1::FieldKind::U8  => FieldValue::U8(0x01),
            fjell_semantic_v1::FieldKind::U64 => FieldValue::U64(0x0102030405060708),
            fjell_semantic_v1::FieldKind::Bytes16 => FieldValue::Bytes16([0xAB; 16]),
            fjell_semantic_v1::FieldKind::Bytes32 => FieldValue::Bytes32([0xCD; 32]),
        })
        .collect();
    let mut buf = vec![0u8; 256];

    let mut group = c.benchmark_group("semantic");
    group.bench_function("encode", |b| {
        b.iter(|| {
            black_box(encode(black_box(entry.tag), 1_000_000, black_box(&fields), &mut buf))
        })
    });

    // Encode once to get a valid byte slice for decode bench.
    let n = encode(entry.tag, 1_000_000, &fields, &mut buf).unwrap();
    let encoded = buf[..n].to_vec();

    group.bench_function("decode", |b| {
        b.iter(|| black_box(decode(black_box(&encoded))))
    });
    group.finish();
}

criterion_group!(benches, bench_encode);
criterion_main!(benches);
