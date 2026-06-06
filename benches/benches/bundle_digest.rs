//! Bundle digest benchmarks (RFC-v0.10-004).
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use fjell_bundle_format::build_bundle;

fn bench_bundle(c: &mut Criterion) {
    let binary = vec![0xEFu8; 4096];  // 4 KiB fake binary
    let manifest_digest = [0xABu8; 16];

    let mut group = c.benchmark_group("bundle");
    group.bench_function("build_bundle/4kib", |b| {
        b.iter(|| {
            black_box(build_bundle(
                black_box("fjell-bench-service"),
                black_box(1),
                black_box(1),
                black_box(1),
                black_box(manifest_digest),
                black_box(&binary),
            ))
        })
    });

    let large = vec![0xEFu8; 1024 * 1024];  // 1 MiB
    group.bench_function("build_bundle/1mib", |b| {
        b.iter(|| {
            black_box(build_bundle(
                "fjell-bench-service", 1, 1, 1, manifest_digest, black_box(&large),
            ))
        })
    });
    group.finish();
}

criterion_group!(benches, bench_bundle);
criterion_main!(benches);
