use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use pmcp::utils::FastJsonParser;
use serde_json::{json, Value};
use std::hint::black_box as hint_black_box;

fn generate_test_json(size: usize) -> Vec<u8> {
    let mut items = Vec::new();
    for i in 0..size {
        items.push(json!({
            "id": i,
            "name": format!("Item {}", i),
            "value": i as f64 * 1.5,
            "active": i % 2 == 0,
            "tags": vec!["tag1", "tag2", "tag3"],
            "metadata": {
                "created": "2024-01-01T00:00:00Z",
                "updated": "2024-01-02T00:00:00Z",
                "version": 1
            }
        }));
    }
    
    let data = json!({
        "items": items,
        "total": size,
        "page": 1,
        "has_more": false
    });
    
    serde_json::to_vec(&data).unwrap()
}

fn benchmark_json_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_parsing");
    
    for size in [10, 100, 1000].iter() {
        let json_data = generate_test_json(*size);
        group.throughput(Throughput::Bytes(json_data.len() as u64));
        
        // Benchmark SIMD-enabled parser
        #[cfg(feature = "simd")]
        group.bench_with_input(
            BenchmarkId::new("simd", size),
            &json_data,
            |b, data| {
                let parser = FastJsonParser::new();
                b.iter(|| {
                    let result = parser.parse(black_box(data));
                    hint_black_box(result)
                })
            },
        );
        
        // Benchmark standard serde_json
        group.bench_with_input(
            BenchmarkId::new("serde_json", size),
            &json_data,
            |b, data| {
                b.iter(|| {
                    let result: Result<Value, _> = serde_json::from_slice(black_box(data));
                    hint_black_box(result)
                })
            },
        );
    }
    
    group.finish();
}

fn benchmark_batch_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_parsing");
    
    for batch_size in [10, 50, 100].iter() {
        let jsons: Vec<Vec<u8>> = (0..*batch_size)
            .map(|i| {
                serde_json::to_vec(&json!({
                    "id": i,
                    "message": format!("Message {}", i),
                    "timestamp": "2024-01-01T00:00:00Z"
                })).unwrap()
            })
            .collect();
        
        let json_refs: Vec<&[u8]> = jsons.iter().map(|v| v.as_slice()).collect();
        
        group.throughput(Throughput::Elements(*batch_size as u64));
        
        // Benchmark SIMD batch parsing
        #[cfg(feature = "simd")]
        group.bench_with_input(
            BenchmarkId::new("simd_batch", batch_size),
            &json_refs,
            |b, data| {
                let parser = FastJsonParser::new();
                b.iter(|| {
                    let results = parser.parse_batch(black_box(data));
                    hint_black_box(results)
                })
            },
        );
        
        // Benchmark sequential parsing
        group.bench_with_input(
            BenchmarkId::new("sequential", batch_size),
            &json_refs,
            |b, data| {
                b.iter(|| {
                    let results: Vec<_> = data.iter()
                        .map(|json| serde_json::from_slice::<Value>(json))
                        .collect();
                    hint_black_box(results)
                })
            },
        );
    }
    
    group.finish();
}

#[cfg(feature = "simd")]
fn benchmark_utf8_validation(c: &mut Criterion) {
    use pmcp::simd::json;
    
    let mut group = c.benchmark_group("utf8_validation");
    
    for size in [100, 1000, 10000].iter() {
        let text = "Hello, 世界! 🦀 ".repeat(size / 20);
        let bytes = text.as_bytes();
        
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        
        // Benchmark SIMD UTF-8 validation
        group.bench_with_input(
            BenchmarkId::new("simd", size),
            &bytes,
            |b, data| {
                b.iter(|| {
                    let valid = unsafe { json::validate_utf8_simd(black_box(data)) };
                    hint_black_box(valid)
                })
            },
        );
        
        // Benchmark standard UTF-8 validation
        group.bench_with_input(
            BenchmarkId::new("standard", size),
            &bytes,
            |b, data| {
                b.iter(|| {
                    let valid = std::str::from_utf8(black_box(data)).is_ok();
                    hint_black_box(valid)
                })
            },
        );
    }
    
    group.finish();
}

#[cfg(feature = "simd")]
fn benchmark_websocket_masking(c: &mut Criterion) {
    use pmcp::simd::serialization;
    
    let mut group = c.benchmark_group("websocket_masking");
    
    for size in [128, 1024, 8192].iter() {
        let mut data = vec![0u8; *size];
        for (i, byte) in data.iter_mut().enumerate() {
            *byte = (i % 256) as u8;
        }
        
        let mask = [0xAA, 0xBB, 0xCC, 0xDD];
        
        group.throughput(Throughput::Bytes(*size as u64));
        
        // Benchmark SIMD XOR masking
        group.bench_with_input(
            BenchmarkId::new("simd", size),
            &data,
            |b, data| {
                b.iter(|| {
                    let mut buffer = data.clone();
                    unsafe { serialization::xor_mask_simd(&mut buffer, mask) };
                    hint_black_box(buffer)
                })
            },
        );
        
        // Benchmark standard XOR masking
        group.bench_with_input(
            BenchmarkId::new("standard", size),
            &data,
            |b, data| {
                b.iter(|| {
                    let mut buffer = data.clone();
                    for (i, byte) in buffer.iter_mut().enumerate() {
                        *byte ^= mask[i % 4];
                    }
                    hint_black_box(buffer)
                })
            },
        );
    }
    
    group.finish();
}

#[cfg(not(feature = "simd"))]
criterion_group!(benches, benchmark_json_parsing, benchmark_batch_parsing);

#[cfg(feature = "simd")]
criterion_group!(
    benches,
    benchmark_json_parsing,
    benchmark_batch_parsing,
    benchmark_utf8_validation,
    benchmark_websocket_masking
);

criterion_main!(benches);