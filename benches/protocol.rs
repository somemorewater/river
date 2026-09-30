//! RESP protocol benchmarks: encode, decode, and pipelined decode.
//!
//! Frames are representative (PING, GET key, SET key value, longer keys +
//! all six frame types), not trivial one-byte inputs. Pipeline decoding is
//! measured on in-memory buffers only — no TCP involved.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use river::protocol::frame::Frame;
use river::protocol::resp::{self, DecodeResult};
use std::hint::black_box;

fn sample_frames() -> Vec<(&'static str, Frame)> {
    vec![
        ("simple", Frame::Simple("PONG".to_string())),
        ("integer", Frame::Integer(1)),
        ("bulk", Frame::Bulk("Water".to_string())),
        (
            "array_ping",
            Frame::Array(vec![Frame::Bulk("PING".to_string())]),
        ),
        (
            "array_get",
            Frame::Array(vec![
                Frame::Bulk("GET".to_string()),
                Frame::Bulk("name".to_string()),
            ]),
        ),
        (
            "array_set",
            Frame::Array(vec![
                Frame::Bulk("SET".to_string()),
                Frame::Bulk("bench:key:000042".to_string()),
                Frame::Bulk("bench:value:000042:xxxxxxxxxxxxxxxx".to_string()),
            ]),
        ),
        (
            "array_get_long_key",
            Frame::Array(vec![
                Frame::Bulk("GET".to_string()),
                Frame::Bulk("bench:key:some-much-longer-key-name-0123456789".to_string()),
            ]),
        ),
        ("error", Frame::Error("ERROR unknown command".to_string())),
        ("null", Frame::Null),
    ]
}

fn bench_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("protocol_encode");
    for (name, frame) in sample_frames() {
        group.bench_with_input(BenchmarkId::new("encode", name), &frame, |b, frame| {
            b.iter(|| {
                black_box(resp::encode(black_box(frame)));
            });
        });
    }
    group.finish();
}

fn bench_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("protocol_decode");
    for (name, frame) in sample_frames() {
        let bytes = resp::encode(&frame);
        group.bench_with_input(BenchmarkId::new("decode", name), &bytes, |b, bytes| {
            b.iter(|| {
                let result = resp::decode(black_box(bytes)).expect("valid frame");
                match result {
                    DecodeResult::Complete(frame, consumed) => {
                        black_box((frame, consumed));
                    }
                    DecodeResult::Incomplete => panic!("sample must decode completely"),
                }
            });
        });
    }
    // Malformed input handling cost (error path, not success path).
    group.bench_function("decode/malformed", |b| {
        let bytes = b"HELLO\r\n".to_vec();
        b.iter(|| {
            let _ = black_box(resp::decode(black_box(&bytes)));
        });
    });
    // Truncated input cost (Incomplete path).
    group.bench_function("decode/incomplete", |b| {
        let bytes = b"*2\r\n$3\r\nGET\r\n".to_vec();
        b.iter(|| {
            let _ = black_box(resp::decode(black_box(&bytes)));
        });
    });
    group.finish();
}

fn bench_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("protocol_pipeline");
    for count in [1usize, 10, 100] {
        let mut buffer = Vec::new();
        for i in 0..count {
            let frame = Frame::Array(vec![
                Frame::Bulk("SET".to_string()),
                Frame::Bulk(format!("bench:key:{i:06}")),
                Frame::Bulk(format!("bench:value:{i:06}")),
            ]);
            buffer.extend_from_slice(&resp::encode(&frame));
        }
        group.bench_with_input(
            BenchmarkId::new("decode_buffer", count),
            &buffer,
            |b, buffer| {
                b.iter(|| {
                    let mut offset = 0usize;
                    let mut frames = 0usize;
                    while offset < buffer.len() {
                        match resp::decode(black_box(&buffer[offset..])).expect("valid") {
                            DecodeResult::Complete(frame, consumed) => {
                                black_box(&frame);
                                offset += consumed;
                                frames += 1;
                            }
                            DecodeResult::Incomplete => panic!("pipeline must be complete"),
                        }
                    }
                    black_box(frames);
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_encode, bench_decode, bench_pipeline);
criterion_main!(benches);
