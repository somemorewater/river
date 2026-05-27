use crate::store::engine::RiverStore;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

#[derive(Debug, Default)]
struct Shard {
    data: HashMap<String, String>,
    expirations: HashMap<String, u64>,
}

#[derive(Debug)]
pub struct ConcurrentStore {
    shards: Vec<RwLock<Shard>>,
    operations: AtomicUsize,
    started_at: Instant,
}

#[derive(Debug, Clone, Copy)]
pub struct StoreStats {
    pub keys: usize,
    pub operations: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct StoreHealth {
    pub status: &'static str,
    pub keys: usize,
    pub operations: usize,
    pub uptime_seconds: u64,
}

impl ConcurrentStore {
    pub fn new(shard_count: usize) -> Self {
        let shard_count = shard_count.max(1);
        let shards = (0..shard_count).map(|_| RwLock::new(Shard::default())).collect();
        Self {
            shards,
            operations: AtomicUsize::new(0),
            started_at: Instant::now(),
        }
    }

    pub fn new_auto() -> Self {
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        // A small multiple of cores keeps contention low without adding too much overhead.
        Self::new((cores * 4).clamp(4, 128))
    }

    pub async fn from_persisted(persisted: RiverStore, shard_count: usize) -> Self {
        let (data, expirations) = persisted.into_maps();
        let store = Self::new(shard_count);

        // Bulk load with per-shard write locks.
        for (key, value) in data {
            let shard_idx = store.shard_for_key(&key);
            let mut shard = store.shards[shard_idx].write().await;
            shard.data.insert(key, value);
        }
        for (key, expires_at) in expirations {
            let shard_idx = store.shard_for_key(&key);
            let mut shard = store.shards[shard_idx].write().await;
            shard.expirations.insert(key, expires_at);
        }

        store
    }

    pub fn shard_count(&self) -> usize {
        self.shards.len()
    }

    pub async fn set(&self, key: String, value: String) {
        self.operations.fetch_add(1, Ordering::Relaxed);
        let shard_idx = self.shard_for_key(&key);
        let mut shard = self.shards[shard_idx].write().await;
        shard.expirations.remove(&key);
        shard.data.insert(key, value);
    }

    pub async fn set_with_expiration(&self, key: String, value: String, seconds: u64) {
        self.operations.fetch_add(1, Ordering::Relaxed);
        let expires_at = current_unix_seconds().saturating_add(seconds);
        let shard_idx = self.shard_for_key(&key);
        let mut shard = self.shards[shard_idx].write().await;
        shard.expirations.insert(key.clone(), expires_at);
        shard.data.insert(key, value);
    }

    pub async fn get(&self, key: &str) -> Option<String> {
        self.operations.fetch_add(1, Ordering::Relaxed);
        let shard_idx = self.shard_for_key(key);

        {
            let shard = self.shards[shard_idx].read().await;
            if let Some(expires_at) = shard.expirations.get(key) {
                if *expires_at <= current_unix_seconds() {
                    // Expired: fall through and remove under a write lock.
                } else {
                    return shard.data.get(key).cloned();
                }
            } else {
                return shard.data.get(key).cloned();
            }
        }

        // If we got here, there was an expiration and it's now expired.
        let mut shard = self.shards[shard_idx].write().await;
        let Some(expires_at) = shard.expirations.get(key).copied() else {
            return shard.data.get(key).cloned();
        };
        if expires_at <= current_unix_seconds() {
            shard.expirations.remove(key);
            shard.data.remove(key);
            None
        } else {
            shard.data.get(key).cloned()
        }
    }

    pub async fn delete(&self, key: &str) {
        self.operations.fetch_add(1, Ordering::Relaxed);
        let shard_idx = self.shard_for_key(key);
        let mut shard = self.shards[shard_idx].write().await;
        shard.expirations.remove(key);
        shard.data.remove(key);
    }

    pub async fn expire(&self, key: &str, seconds: u64) -> bool {
        self.operations.fetch_add(1, Ordering::Relaxed);
        let shard_idx = self.shard_for_key(key);
        let mut shard = self.shards[shard_idx].write().await;

        if let Some(expires_at) = shard.expirations.get(key).copied() {
            if expires_at <= current_unix_seconds() {
                shard.expirations.remove(key);
                shard.data.remove(key);
                return false;
            }
        }

        if !shard.data.contains_key(key) {
            return false;
        }

        let expires_at = current_unix_seconds().saturating_add(seconds);
        shard.expirations.insert(key.to_string(), expires_at);
        true
    }

    pub async fn cleanup_expired(&self) -> usize {
        let now = current_unix_seconds();
        let mut removed = 0usize;

        for shard_lock in &self.shards {
            let mut shard = shard_lock.write().await;
            let expired_keys: Vec<String> = shard
                .expirations
                .iter()
                .filter_map(|(key, expires_at)| (*expires_at <= now).then(|| key.clone()))
                .collect();
            removed += expired_keys.len();
            for key in expired_keys {
                shard.expirations.remove(&key);
                shard.data.remove(&key);
            }
        }

        removed
    }

    pub async fn stats(&self) -> StoreStats {
        let keys = self.key_count().await;
        StoreStats {
            keys,
            operations: self.operations.load(Ordering::Relaxed),
        }
    }

    pub async fn health(&self) -> StoreHealth {
        let keys = self.key_count().await;
        StoreHealth {
            status: "OK",
            keys,
            operations: self.operations.load(Ordering::Relaxed),
            uptime_seconds: self.started_at.elapsed().as_secs(),
        }
    }

    pub async fn snapshot(&self) -> RiverStore {
        let mut data = HashMap::new();
        let mut expirations = HashMap::new();

        for shard_lock in &self.shards {
            let shard = shard_lock.read().await;
            data.extend(shard.data.clone());
            expirations.extend(shard.expirations.clone());
        }

        RiverStore::from_maps(data, expirations)
    }

    async fn key_count(&self) -> usize {
        let now = current_unix_seconds();
        let mut keys = 0usize;

        // Count only non-expired keys; we avoid mutating the store here.
        for shard_lock in &self.shards {
            let shard = shard_lock.read().await;
            for key in shard.data.keys() {
                if let Some(expires_at) = shard.expirations.get(key) {
                    if *expires_at <= now {
                        continue;
                    }
                }
                keys += 1;
            }
        }

        keys
    }

    fn shard_for_key(&self, key: &str) -> usize {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % self.shards.len()
    }
}

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::ConcurrentStore;

    #[tokio::test]
    async fn concurrent_gets_do_not_block_each_other() {
        let store = ConcurrentStore::new(16);
        store
            .set("name".to_string(), "river".to_string())
            .await;

        let store = std::sync::Arc::new(store);
        let mut tasks = Vec::new();
        for _ in 0..32 {
            let store = std::sync::Arc::clone(&store);
            tasks.push(tokio::spawn(async move {
                for _ in 0..100 {
                    assert_eq!(store.get("name").await.as_deref(), Some("river"));
                }
            }));
        }

        for task in tasks {
            task.await.expect("task should not panic");
        }
    }

    #[tokio::test]
    async fn expired_keys_disappear_on_get() {
        let store = ConcurrentStore::new(4);
        store
            .set_with_expiration("session".to_string(), "abc".to_string(), 0)
            .await;
        assert_eq!(store.get("session").await, None);
        let stats = store.stats().await;
        assert_eq!(stats.keys, 0);
    }
}
