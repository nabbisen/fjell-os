//! Audit record encode benchmarks (RFC-v0.10-004).
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use fjell_audit_format::{AuditKind, AuditRecordBin};

fn bench_audit(c: &mut Criterion) {
    let record = AuditRecordBin {
        seq: 1, tick: 0x0001_0000,
        kind: AuditKind::Syscall as u16,
        task: 0, arg0: 42, arg1: 0, result: 0,
    };
    let mut buf = [0u8; 32];
    // Encode to bytes manually for the from_bytes bench
    buf[..8].copy_from_slice(&record.seq.to_le_bytes());
    buf[8..16].copy_from_slice(&record.tick.to_le_bytes());
    buf[16..18].copy_from_slice(&record.kind.to_le_bytes());

    let mut group = c.benchmark_group("audit");
    group.bench_function("from_bytes", |b| {
        b.iter(|| black_box(AuditRecordBin::from_bytes(black_box(&buf))))
    });
    group.finish();
}

criterion_group!(benches, bench_audit);
criterion_main!(benches);
