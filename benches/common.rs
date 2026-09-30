//! Shared deterministic helpers for River's Criterion benches.
//!
//! Each file in `benches/` compiles as its own crate, so bench targets
//! include this file with `#[path = "common.rs"] mod common;`.
//! Nothing here measures anything; it only builds keys and runs workloads
//! so the timed closures stay clean (no setup/printing inside measurement).

#![allow(dead_code)]

use river::store::engine::RiverStore;
use river::store::shared::ConcurrentStore;
use std::sync::Arc;

pub const KEY_SPACE: usize = 10_000;
pub const OPS_PER_TASK: usize = 200;

pub fn key(i: usize) -> String {
    format!("bench:key:{i:06}")
}

pub fn value(i: usize) -> String {
    format!("bench:value:{i:06}:xxxxxxxxxxxxxxxx")
}

/// Deterministic pseudo-random pick in `[0, key_space)` without RNG overhead
/// in the timed section (cheap LCG step on the task's counter).
pub fn pick(counter: usize, key_space: usize) -> usize {
    (counter.wrapping_mul(1_103_515_245).wrapping_add(12_345)) % key_space
}

pub fn seed_river_store(n: usize) -> RiverStore {
    let mut store = RiverStore::new();
    for i in 0..n {
        store.set(key(i), value(i));
    }
    store
}

pub async fn seed_concurrent(store: &ConcurrentStore, n: usize) {
    for i in 0..n {
        store.set(key(i), value(i)).await;
    }
}

pub fn new_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("bench runtime")
}

/// GET-heavy concurrent workload over a pre-seeded key space.
pub async fn run_get_workload(store: Arc<ConcurrentStore>, tasks: usize, key_space: usize) {
    let mut handles = Vec::with_capacity(tasks);
    for t in 0..tasks {
        let store = Arc::clone(&store);
        handles.push(tokio::spawn(async move {
            for j in 0..OPS_PER_TASK {
                let k = key((t * 997 + j) % key_space);
                let _ = store.get(&k).await;
            }
        }));
    }
    for h in handles {
        let _ = h.await;
    }
}

/// SET-heavy concurrent workload; each task writes mostly-distinct keys so
/// shards share the write load instead of hammering one key.
pub async fn run_set_workload(store: Arc<ConcurrentStore>, tasks: usize, salt: usize) {
    let mut handles = Vec::with_capacity(tasks);
    for t in 0..tasks {
        let store = Arc::clone(&store);
        handles.push(tokio::spawn(async move {
            for j in 0..OPS_PER_TASK {
                let i = (salt + t * OPS_PER_TASK + j) % KEY_SPACE;
                store.set(key(i), value(i)).await;
            }
        }));
    }
    for h in handles {
        let _ = h.await;
    }
}

/// Mixed workload with explicit ratios: `get_pct + set_pct + del_pct == 100`.
/// GETs hit the seeded space; SETs/DELs use a disjoint scratch range so DELs
/// do not starve later GETs of hits.
pub async fn run_mixed_workload(
    store: Arc<ConcurrentStore>,
    tasks: usize,
    get_pct: usize,
    set_pct: usize,
) {
    debug_assert_eq!(get_pct + set_pct + (100 - get_pct - set_pct), 100);
    let del_pct = 100 - get_pct - set_pct;
    let mut handles = Vec::with_capacity(tasks);
    for t in 0..tasks {
        let store = Arc::clone(&store);
        handles.push(tokio::spawn(async move {
            for j in 0..OPS_PER_TASK {
                let r = pick(t * OPS_PER_TASK + j, 100);
                if r < get_pct {
                    let k = key((t * 997 + j) % KEY_SPACE);
                    let _ = store.get(&k).await;
                } else if r < get_pct + set_pct {
                    let i = KEY_SPACE + (t * OPS_PER_TASK + j) % KEY_SPACE;
                    store.set(key(i), value(i)).await;
                } else {
                    debug_assert!(del_pct > 0);
                    let i = KEY_SPACE + (t * 571 + j) % KEY_SPACE;
                    let k = key(i);
                    store.delete(&k).await;
                }
            }
        }));
    }
    for h in handles {
        let _ = h.await;
    }
}
