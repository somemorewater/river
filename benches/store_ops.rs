//! Core store-operation benchmarks: SET / GET / DEL.
//!
//! - `store_set`: insert-new vs update-existing, RiverStore vs ConcurrentStore.
//! - `store_get`: existing-key hit vs missing-key miss (single-threaded).
//! - `store_delete`: existing key vs missing key.
//!
//! Setup (seeding, key generation) stays outside the timed operation via
//! `iter_batched`; only the single op under test is measured.

#[path = "common.rs"]
mod common;

use common::{KEY_SPACE, key, seed_river_store, value};
use criterion::{Criterion, criterion_group, criterion_main};
use river::store::engine::RiverStore;
use river::store::shared::ConcurrentStore;
use std::hint::black_box;
use std::sync::Arc;

fn bench_set(c: &mut Criterion) {
    let mut group = c.benchmark_group("store_set");

    // Insert-new: each iteration gets a fresh store + a never-before-seen key,
    // so we measure insertion rather than overwrite.
    group.bench_function("riverstore/insert_new", |b| {
        let mut counter = 0usize;
        b.iter_batched(
            || {
                counter += 1;
                // Fresh store per batch element is expensive for large seeds;
                // use an empty store and a unique key: pure insertion cost.
                (RiverStore::new(), key(KEY_SPACE + counter))
            },
            |(mut store, k)| {
                store.set(black_box(k), black_box(value(0)));
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Update-existing: pre-seeded store, rewrite one live key.
    group.bench_function("riverstore/update_existing", |b| {
        let mut store = seed_river_store(KEY_SPACE);
        let k = key(42);
        b.iter(|| {
            store.set(black_box(k.clone()), black_box(value(1)));
        });
    });

    // ConcurrentStore insert-new (single-threaded access; concurrency is
    // covered by store_workloads). Fresh keys via counter; runtime reused.
    group.bench_function("concurrent/insert_new", |b| {
        let rt = common::new_runtime();
        let store = Arc::new(ConcurrentStore::new(8));
        let mut counter = 0usize;
        b.iter_batched(
            || {
                counter += 1;
                key(KEY_SPACE + counter)
            },
            |k| {
                rt.block_on(async {
                    store.set(black_box(k), black_box(value(0))).await;
                });
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("concurrent/update_existing", |b| {
        let rt = common::new_runtime();
        let store = Arc::new(ConcurrentStore::new(8));
        rt.block_on(common::seed_concurrent(&store, KEY_SPACE));
        let k = key(42);
        b.iter(|| {
            rt.block_on(async {
                store.set(black_box(k.clone()), black_box(value(1))).await;
            });
        });
    });

    group.finish();
}

fn bench_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("store_get");

    group.bench_function("riverstore/hit", |b| {
        let mut store = seed_river_store(KEY_SPACE);
        let k = key(7);
        b.iter(|| {
            black_box(store.get(black_box(&k)));
        });
    });

    group.bench_function("riverstore/miss", |b| {
        let mut store = seed_river_store(KEY_SPACE);
        let k = key(KEY_SPACE + 1);
        b.iter(|| {
            black_box(store.get(black_box(&k)));
        });
    });

    group.bench_function("concurrent/hit", |b| {
        let rt = common::new_runtime();
        let store = Arc::new(ConcurrentStore::new(8));
        rt.block_on(common::seed_concurrent(&store, KEY_SPACE));
        let k = key(7);
        b.iter(|| {
            rt.block_on(async {
                black_box(store.get(black_box(&k)).await);
            });
        });
    });

    group.bench_function("concurrent/miss", |b| {
        let rt = common::new_runtime();
        let store = Arc::new(ConcurrentStore::new(8));
        rt.block_on(common::seed_concurrent(&store, KEY_SPACE));
        let k = key(KEY_SPACE + 1);
        b.iter(|| {
            rt.block_on(async {
                black_box(store.get(black_box(&k)).await);
            });
        });
    });

    group.finish();
}

fn bench_delete(c: &mut Criterion) {
    let mut group = c.benchmark_group("store_delete");

    // Deleting an existing key needs a fresh present key per iteration.
    group.bench_function("riverstore/existing", |b| {
        let mut counter = 0usize;
        b.iter_batched(
            || {
                let mut store = RiverStore::new();
                let k = key(counter);
                counter += 1;
                store.set(k.clone(), value(0));
                (store, k)
            },
            |(mut store, k)| {
                store.delete(black_box(&k));
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("riverstore/missing", |b| {
        let mut store = seed_river_store(1_000);
        let k = key(KEY_SPACE + 999);
        b.iter(|| {
            store.delete(black_box(&k));
        });
    });

    group.bench_function("concurrent/existing", |b| {
        let rt = common::new_runtime();
        let store = Arc::new(ConcurrentStore::new(8));
        let mut counter = 0usize;
        b.iter_batched(
            || {
                let k = key(counter);
                counter += 1;
                rt.block_on(async {
                    store.set(k.clone(), value(0)).await;
                });
                k
            },
            |k| {
                rt.block_on(async {
                    store.delete(black_box(&k)).await;
                });
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("concurrent/missing", |b| {
        let rt = common::new_runtime();
        let store = Arc::new(ConcurrentStore::new(8));
        rt.block_on(common::seed_concurrent(&store, 1_000));
        let k = key(KEY_SPACE + 999);
        b.iter(|| {
            rt.block_on(async {
                store.delete(black_box(&k)).await;
            });
        });
    });

    group.finish();
}

criterion_group!(benches, bench_set, bench_get, bench_delete);
criterion_main!(benches);
