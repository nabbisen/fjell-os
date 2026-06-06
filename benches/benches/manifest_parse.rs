//! CapManifest parse and lint benchmarks (RFC-v0.10-004).
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use fjell_cap_manifest::{parse_manifest, lint_manifest};

const SAMPLE: &str = "\
service     = \"fjell-bench-service\"
sdk_api_rev = 1
caps        = [\"Endpoint\", \"AuditDrain\", \"PersistentStore\"]
rights      = [\"SEND\", \"RECV\", \"AUDIT_DRAIN\", \"READ\", \"WRITE\"]
ipc_tags    = [\"v0_7::SYNC_ENVELOPE\", \"tags::READY\"]
intents     = [0x0101, 0x0102, 0x0201, 0x0301]
";

fn bench_manifest(c: &mut Criterion) {
    let mut group = c.benchmark_group("manifest");

    group.bench_function("parse", |b| {
        b.iter(|| black_box(parse_manifest(black_box(SAMPLE))))
    });

    let parsed = parse_manifest(SAMPLE).unwrap();
    group.bench_function("lint", |b| {
        b.iter(|| black_box(lint_manifest(black_box(&parsed), black_box(1))))
    });

    group.bench_function("parse_and_lint", |b| {
        b.iter(|| {
            let m = parse_manifest(black_box(SAMPLE)).unwrap();
            black_box(lint_manifest(&m, 1))
        })
    });

    group.finish();
}

criterion_group!(benches, bench_manifest);
criterion_main!(benches);
