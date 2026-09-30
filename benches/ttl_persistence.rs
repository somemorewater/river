//! TTL and persistence benchmarks.
//!
//! TTL uses TTL 0 for "already expired" (no real-second waits; measures the
//! expiration logic, not the 1s background scheduler). Persistence separates
//! in-memory serialization cost from filesystem I/O, uses temp files that are
//! cleaned up, and never touches the project's real `river.db`.

#[path = "common.rs"]
mod common;

use common::{KEY_SPACE, key, seed_river_store, value};
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use river::persistence::storage;
use river::store::engine::RiverStore;
use river::store::shared::ConcurrentStore;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn tmp_db_path(tag: &str) -> std::path::PathBuf {
    let id = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "river-bench-{}-{}-{}",
        std::process::id(),
        tag,
        id
    ));
    std::fs::create_dir_all(&dir).expect("bench temp dir");
    dir.join("bench.db")
}

fn bench_ttl(c: &mut Criterion) {
    let mut group = c.benchmark_group("ttl");

    // SETEX insert.
    group.bench_function("riverstore/setex", |b| {
        let mut counter = 0usize;
        b.iter_batched(
            || {
                counter += 1;
                (RiverStore::new(), key(KEY_SPACE + counter))
            },
            |(mut store, k)| {
                store.set_with_expiration(black_box(k), black_box(value(0)), black_box(60));
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // EXPIRE on a live key.
    group.bench_function("riverstore/expire_hit", |b| {
        let mut store = seed_river_store(1_000);
        let k = key(3);
        b.iter(|| {
            black_box(store.expire(black_box(&k), black_box(60)));
        });
    });

    group.bench_function("riverstore/expire_miss", |b| {
        let mut store = seed_river_store(1_000);
        let k = key(KEY_SPACE + 5);
        b.iter(|| {
            black_box(store.expire(black_box(&k), black_box(60)));
        });
    });

    // GET on a live (non-expired) key with TTL attached.
    group.bench_function("riverstore/get_live_ttl", |b| {
        let mut store = RiverStore::new();
        store.set_with_expiration(key(0), value(0), 3_600);
        b.iter(|| {
            black_box(store.get(black_box("bench:key:000000")));
        });
    });

    // GET on an expired key (TTL 0 expires immediately by wall-clock).
    group.bench_function("riverstore/get_expired", |b| {
        b.iter_batched(
            || {
                let mut store = RiverStore::new();
                store.set_with_expiration(key(0), value(0), 0);
                store
            },
            |mut store| {
                black_box(store.get(black_box("bench:key:000000")));
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // cleanup_expired over N pre-expired keys (setup outside timing).
    group.bench_function("riverstore/cleanup_1000_expired", |b| {
        b.iter_batched(
            || {
                let mut store = RiverStore::new();
                for i in 0..1_000usize {
                    store.set_with_expiration(key(i), value(i), 0);
                }
                store
            },
            |mut store| {
                black_box(store.cleanup_expired());
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Async ConcurrentStore equivalents (single-threaded access).
    group.bench_function("concurrent/get_live_ttl", |b| {
        let rt = common::new_runtime();
        let store = Arc::new(ConcurrentStore::new(8));
        rt.block_on(async {
            store
                .set_with_expiration(key(0), value(0), 3_600)
                .await;
        });
        b.iter(|| {
            rt.block_on(async {
                black_box(store.get(black_box("bench:key:000000")).await);
            });
        });
    });

    group.bench_function("concurrent/cleanup_1000_expired", |b| {
        let rt = common::new_runtime();
        let store = Arc::new(ConcurrentStore::new(8));
        b.iter_batched(
            || {
                rt.block_on(async {
                    for i in 0..1_000usize {
                        store.set_with_expiration(key(i), value(i), 0).await;
                    }
                });
            },
            |_| {
                rt.block_on(async {
                    black_box(store.cleanup_expired().await);
                });
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_persistence(c: &mut Criterion) {
    let mut group = c.benchmark_group("persistence");

    // Snapshot: ConcurrentStore -> RiverStore (in-memory copy, no disk).
    group.bench_function("snapshot_10k", |b| {
        let rt = common::new_runtime();
        let store = Arc::new(ConcurrentStore::new(8));
        rt.block_on(common::seed_concurrent(&store, KEY_SPACE));
        b.iter(|| {
            rt.block_on(async {
                black_box(store.snapshot().await);
            });
        });
    });

    // In-memory serialization cost only.
    group.bench_function("serialize_10k", |b| {
        let store = seed_river_store(KEY_SPACE);
        b.iter(|| {
            let bytes = bincode::serialize(black_box(&store)).expect("serialize");
            black_box(bytes);
        });
    });

    // Deserialization cost (bytes prepared once outside timing).
    group.bench_function("deserialize_10k", |b| {
        let store = seed_river_store(KEY_SPACE);
        let bytes = bincode::serialize(&store).expect("serialize");
        b.iter(|| {
            let restored: RiverStore =
                bincode::deserialize(black_box(&bytes)).expect("deserialize");
            black_box(restored);
        });
    });

    // Full save-to-disk through the real storage helper (temp file, cleaned).
    group.bench_function("save_to_disk_10k", |b| {
        let store = seed_river_store(KEY_SPACE);
        let path = tmp_db_path("save");
        b.iter(|| {
            storage::save_to_disk(black_box(&store), black_box(&path)).expect("save");
        });
        let _ = std::fs::remove_dir_all(path.parent().expect("tmp parent"));
    });

    // Load-from-disk (file written once outside timing, read per iteration).
    group.bench_function("load_from_disk_10k", |b| {
        let store = seed_river_store(KEY_SPACE);
        let path = tmp_db_path("load");
        storage::save_to_disk(&store, &path).expect("seed save");
        b.iter(|| {
            let loaded = storage::load_from_disk(black_box(&path)).expect("load");
            black_box(loaded);
        });
        let _ = std::fs::remove_dir_all(path.parent().expect("tmp parent"));
    });

    group.finish();
}

criterion_group!(benches, bench_ttl, bench_persistence);
criterion_main!(benches);
