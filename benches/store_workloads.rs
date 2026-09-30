//! Sequential-vs-concurrent and shard-scaling benchmarks.
//!
//! Groups:
//! - `store_concurrent_get` (read contention: 1/4/16/64 readers, 10k keys)
//! - `store_concurrent_write` (write contention: 1/4/16/64 writers)
//! - `store_mixed_workload` (70/20/10 realistic + 50/40/10 write-heavy)
//! - `store_shard_scaling` (shards 1/2/4/8/16/32, 16 tasks, GET-heavy)
//!
//! Shard range rationale: the machine behavior is reported by Criterion, but
//! 1..32 brackets single-lock contention vs overhead; 32 matches ~4x cores on
//! an 8-core box, the same ceiling `ConcurrentStore::new_auto` uses.

#[path = "common.rs"]
mod common;

use common::{KEY_SPACE, run_get_workload, run_mixed_workload, run_set_workload};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use river::store::shared::ConcurrentStore;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn seeded(shards: usize) -> (tokio::runtime::Runtime, Arc<ConcurrentStore>) {
    let rt = common::new_runtime();
    let store = Arc::new(ConcurrentStore::new(shards));
    rt.block_on(common::seed_concurrent(&store, KEY_SPACE));
    (rt, store)
}

fn bench_concurrent<F>(c: &mut Criterion, group_name: &'static str, workload: F)
where
    F: Fn(Arc<ConcurrentStore>, usize) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>>
        + Copy,
{
    let mut group = c.benchmark_group(group_name);
    group.measurement_time(Duration::from_secs(5));
    for tasks in [1usize, 4, 16, 64] {
        group.bench_with_input(BenchmarkId::new("tasks", tasks), &tasks, |b, &tasks| {
            let rt = common::new_runtime();
            b.iter_custom(|iters| {
                let (_, store) = seeded(16);
                let start = Instant::now();
                rt.block_on(async {
                    for _ in 0..iters {
                        workload(Arc::clone(&store), tasks).await;
                    }
                });
                start.elapsed()
            });
        });
    }
    group.finish();
}

fn bench_concurrent_get(c: &mut Criterion) {
    bench_concurrent(c, "store_concurrent_get", |store, tasks| {
        Box::pin(run_get_workload(store, tasks, KEY_SPACE))
    });
}

fn bench_concurrent_write(c: &mut Criterion) {
    bench_concurrent(c, "store_concurrent_write", |store, tasks| {
        Box::pin(run_set_workload(store, tasks, 0))
    });
}

fn bench_mixed(c: &mut Criterion) {
    let mut group = c.benchmark_group("store_mixed_workload");
    group.measurement_time(Duration::from_secs(5));
    // (name, get_pct, set_pct); del_pct is the remainder.
    for (name, get_pct, set_pct) in [("get70/set20/del10", 70, 20), ("get50/set40/del10", 50, 40)] {
        for tasks in [1usize, 4, 16, 64] {
            group.bench_with_input(
                BenchmarkId::new(name, tasks),
                &tasks,
                |b, &tasks| {
                    let rt = common::new_runtime();
                    b.iter_custom(|iters| {
                        let (_, store) = seeded(16);
                        let start = Instant::now();
                        rt.block_on(async {
                            for _ in 0..iters {
                                run_mixed_workload(
                                    Arc::clone(&store),
                                    tasks,
                                    get_pct,
                                    set_pct,
                                )
                                .await;
                            }
                        });
                        start.elapsed()
                    });
                },
            );
        }
    }
    group.finish();
}

fn bench_shards(c: &mut Criterion) {
    let mut group = c.benchmark_group("store_shard_scaling");
    group.measurement_time(Duration::from_secs(5));
    // Fixed 16-task GET-heavy workload; only the shard count varies.
    for shards in [1usize, 2, 4, 8, 16, 32] {
        group.bench_with_input(BenchmarkId::new("shards", shards), &shards, |b, &shards| {
            let rt = common::new_runtime();
            b.iter_custom(|iters| {
                let (_, store) = seeded(shards);
                let start = Instant::now();
                rt.block_on(async {
                    for _ in 0..iters {
                        run_get_workload(Arc::clone(&store), 16, KEY_SPACE).await;
                    }
                });
                start.elapsed()
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_concurrent_get,
    bench_concurrent_write,
    bench_mixed,
    bench_shards
);
criterion_main!(benches);
