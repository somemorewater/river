use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use river::store::engine::RiverStore;
use river::store::shared::ConcurrentStore;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

fn bench_parallel_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_get");
    group.measurement_time(Duration::from_secs(5));

    for clients in [1usize, 4, 16, 64] {
        group.bench_with_input(
            BenchmarkId::new("mutex_riverstore", clients),
            &clients,
            |b, &clients| {
                let rt = tokio::runtime::Runtime::new().expect("runtime");
                b.iter_custom(|iters| {
                    let store = Arc::new(Mutex::new(seed_store()));
                    let start = Instant::now();
                    rt.block_on(async {
                        for _ in 0..iters {
                            run_get_workload_mutex(Arc::clone(&store), clients).await;
                        }
                    });
                    start.elapsed()
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("sharded_rwlock", clients),
            &clients,
            |b, &clients| {
                let rt = tokio::runtime::Runtime::new().expect("runtime");
                b.iter_custom(|iters| {
                    let store = Arc::new(ConcurrentStore::new(64));
                    let start = Instant::now();
                    rt.block_on(async {
                        seed_concurrent(&store).await;
                        for _ in 0..iters {
                            run_get_workload_sharded(Arc::clone(&store), clients).await;
                        }
                    });
                    start.elapsed()
                });
            },
        );
    }

    group.finish();
}

fn seed_store() -> RiverStore {
    let mut store = RiverStore::new();
    for i in 0..10_000usize {
        store.set(format!("k{i}"), format!("v{i}"));
    }
    store
}

async fn seed_concurrent(store: &ConcurrentStore) {
    for i in 0..10_000usize {
        store.set(format!("k{i}"), format!("v{i}")).await;
    }
}

async fn run_get_workload_mutex(store: Arc<Mutex<RiverStore>>, clients: usize) {
    let mut tasks = Vec::with_capacity(clients);
    for client_id in 0..clients {
        let store = Arc::clone(&store);
        tasks.push(tokio::spawn(async move {
            for j in 0..200usize {
                let key = format!("k{}", (client_id * 997 + j) % 10_000);
                let mut store = store.lock().await;
                let _ = store.get(&key);
            }
        }));
    }
    for task in tasks {
        let _ = task.await;
    }
}

async fn run_get_workload_sharded(store: Arc<ConcurrentStore>, clients: usize) {
    let mut tasks = Vec::with_capacity(clients);
    for client_id in 0..clients {
        let store = Arc::clone(&store);
        tasks.push(tokio::spawn(async move {
            for j in 0..200usize {
                let key = format!("k{}", (client_id * 997 + j) % 10_000);
                let _ = store.get(&key).await;
            }
        }));
    }
    for task in tasks {
        let _ = task.await;
    }
}

criterion_group!(benches, bench_parallel_get);
criterion_main!(benches);
