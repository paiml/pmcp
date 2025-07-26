//! Simple test benchmark to verify benchmark infrastructure works

use criterion::{criterion_group, criterion_main, Criterion};
use serde_json::json;
use std::hint::black_box;

fn bench_simple_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("simple");

    let simple_request = json!({
        "method": "ping",
        "params": {}
    });

    group.bench_function("serialize_json", |b| {
        b.iter(|| {
            let serialized = serde_json::to_string(&black_box(&simple_request)).unwrap();
            black_box(serialized)
        })
    });

    group.finish();
}

criterion_group!(simple_benches, bench_simple_serialization);
criterion_main!(simple_benches);
