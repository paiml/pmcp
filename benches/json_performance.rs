//! Basic JSON serialization benchmarks for MCP SDK
//!
//! These benchmarks test fundamental JSON operations that are core
//! to the MCP protocol performance.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use serde_json::json;

/// Benchmark basic JSON serialization operations
fn bench_json_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_serialization");

    // Test different sized JSON objects
    let small_json = json!({"method": "ping", "params": {}});
    let medium_json = json!({
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {"tools": {}},
            "clientInfo": {"name": "test", "version": "1.0.0"}
        }
    });
    let large_json = json!({
        "method": "tools/call",
        "params": {
            "name": "complex_tool",
            "arguments": {
                "data": (0..100).map(|i| format!("item_{}", i)).collect::<Vec<_>>(),
                "options": {
                    "format": "json",
                    "include_metadata": true,
                    "filters": ["filter1", "filter2", "filter3"]
                }
            }
        }
    });

    let test_cases = vec![
        ("small", &small_json),
        ("medium", &medium_json),
        ("large", &large_json),
    ];

    for (name, data) in test_cases {
        group.bench_with_input(BenchmarkId::new("serialize", name), &data, |b, data| {
            b.iter(|| {
                let serialized = serde_json::to_string(black_box(data)).unwrap();
                black_box(serialized)
            })
        });

        // Also benchmark pretty serialization
        group.bench_with_input(
            BenchmarkId::new("serialize_pretty", name),
            &data,
            |b, data| {
                b.iter(|| {
                    let serialized = serde_json::to_string_pretty(black_box(data)).unwrap();
                    black_box(serialized)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark JSON deserialization
fn bench_json_deserialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_deserialization");

    let small_str = r#"{"method":"ping","params":{}}"#;
    let medium_str = r#"{"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"clientInfo":{"name":"test","version":"1.0.0"}}}"#;
    let large_str = serde_json::to_string(&json!({
        "method": "tools/call",
        "params": {
            "name": "complex_tool",
            "arguments": {
                "data": (0..100).map(|i| format!("item_{}", i)).collect::<Vec<_>>(),
                "options": {
                    "format": "json",
                    "include_metadata": true,
                    "filters": ["filter1", "filter2", "filter3"]
                }
            }
        }
    }))
    .unwrap();

    let test_cases = vec![
        ("small", small_str),
        ("medium", medium_str),
        ("large", large_str.as_str()),
    ];

    for (name, data) in test_cases {
        group.bench_with_input(BenchmarkId::new("deserialize", name), &data, |b, data| {
            b.iter(|| {
                let parsed: serde_json::Value = serde_json::from_str(black_box(data)).unwrap();
                black_box(parsed)
            })
        });
    }

    group.finish();
}

/// Benchmark JSON roundtrip operations
fn bench_json_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_roundtrip");

    let test_data = json!({
        "jsonrpc": "2.0",
        "id": 42,
        "method": "tools/call",
        "params": {
            "name": "benchmark_tool",
            "arguments": {
                "input": "test data for benchmarking",
                "count": 123,
                "enabled": true,
                "metadata": {
                    "timestamp": "2024-01-01T00:00:00Z",
                    "version": "1.0.0"
                }
            }
        }
    });

    group.bench_function("serialize_deserialize", |b| {
        b.iter(|| {
            let serialized = serde_json::to_string(black_box(&test_data)).unwrap();
            let deserialized: serde_json::Value = serde_json::from_str(&serialized).unwrap();
            black_box(deserialized)
        })
    });

    group.finish();
}

/// Benchmark content-length calculation (important for MCP transport)
fn bench_content_length(c: &mut Criterion) {
    let mut group = c.benchmark_group("content_length");

    let medium_msg = "A".repeat(1000);
    let large_msg = "X".repeat(10000);
    let messages = vec![
        ("small", "{}"),
        ("medium", medium_msg.as_str()),
        ("large", large_msg.as_str()),
    ];

    for (name, message) in messages {
        group.bench_with_input(
            BenchmarkId::new("calculate", name),
            &message,
            |b, message| {
                b.iter(|| {
                    let length = black_box(message).len();
                    black_box(length)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("format_header", name),
            &message,
            |b, message| {
                b.iter(|| {
                    let length = black_box(message).len();
                    let header = format!("Content-Length: {}\r\n\r\n", length);
                    black_box(header)
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    json_benches,
    bench_json_serialization,
    bench_json_deserialization,
    bench_json_roundtrip,
    bench_content_length
);

criterion_main!(json_benches);
