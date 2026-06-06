//! Capability operation benchmarks (RFC-v0.10-004).
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use fjell_cap::{CapHandle, CapRights, CapKind};
use fjell_cap::cspace::CSpace;
use fjell_cap::enforcement::require_cap;
use fjell_cap::slot::NoLease;

fn bench_require_cap(c: &mut Criterion) {
    let mut cs = CSpace::new();
    let h = cs.install_root(CapKind::AuditDrain, 1, CapRights::AUDIT_DRAIN).unwrap();

    let mut group = c.benchmark_group("cap");
    group.bench_function("require_cap/ok", |b| {
        b.iter(|| {
            black_box(require_cap(
                black_box(&cs),
                black_box(h),
                CapKind::AuditDrain,
                CapRights::AUDIT_DRAIN,
                None,
                &NoLease,
            ))
        })
    });
    group.bench_function("require_cap/wrong_handle", |b| {
        b.iter(|| {
            black_box(require_cap(
                black_box(&cs),
                black_box(CapHandle(99)),
                CapKind::AuditDrain,
                CapRights::AUDIT_DRAIN,
                None,
                &NoLease,
            ))
        })
    });
    group.finish();
}

criterion_group!(benches, bench_require_cap);
criterion_main!(benches);
