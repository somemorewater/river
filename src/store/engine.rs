use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize)]
pub struct RiverStore {
    data: HashMap<String, String>,
    expirations: HashMap<String, u64>,
    #[serde(skip)]
    operations: usize,
}

pub struct StoreStats {
    pub keys: usize,
    pub operations: usize,
}

pub struct StoreHealth {
    pub status: &'static str,
    pub keys: usize,
    pub operations: usize,
    pub uptime: usize,
}

impl RiverStore {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
            expirations: HashMap::new(),
            operations: 0,
        }
    }

    pub fn set(&mut self, key: String, value: String) {
        self.operations += 1;
        self.expirations.remove(&key);
        self.data.insert(key, value);
    }

    pub fn set_with_expiration(&mut self, key: String, value: String, seconds: u64) {
        self.operations += 1;
        let expires_at = current_unix_seconds().saturating_add(seconds);
        self.expirations.insert(key.clone(), expires_at);
        self.data.insert(key, value);
    }

    pub fn get(&mut self, key: &str) -> Option<&String> {
        self.operations += 1;
        self.remove_if_expired(key);
        self.data.get(key)
    }

    pub fn delete(&mut self, key: &str) {
        self.operations += 1;
        self.expirations.remove(key);
        self.data.remove(key);
    }

    pub fn expire(&mut self, key: &str, seconds: u64) -> bool {
        self.operations += 1;
        self.remove_if_expired(key);
        if !self.data.contains_key(key) {
            return false;
        }

        let expires_at = current_unix_seconds().saturating_add(seconds);
        self.expirations.insert(key.to_string(), expires_at);
        true
    }

    pub fn cleanup_expired(&mut self) -> usize {
        let now = current_unix_seconds();
        let expired_keys: Vec<String> = self
            .expirations
            .iter()
            .filter_map(|(key, expires_at)| {
                if *expires_at <= now {
                    Some(key.clone())
                } else {
                    None
                }
            })
            .collect();

        let removed = expired_keys.len();
        for key in expired_keys {
            self.expirations.remove(&key);
            self.data.remove(&key);
        }

        removed
    }

    pub fn cleanup_expired_on_startup(&mut self) {
        self.cleanup_expired();
    }

    pub fn stats(&mut self) -> StoreStats {
        self.cleanup_expired();
        StoreStats {
            keys: self.data.len(),
            operations: self.operations,
        }
    }

    pub fn health(&mut self) -> StoreHealth {
        self.cleanup_expired();
        StoreHealth {
            status: "OK",
            keys: self.data.len(),
            operations: self.operations,
            uptime: 0,
        }
    }

    fn remove_if_expired(&mut self, key: &str) {
        let Some(expires_at) = self.expirations.get(key) else {
            return;
        };

        if *expires_at <= current_unix_seconds() {
            self.expirations.remove(key);
            self.data.remove(key);
        }
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
    use super::RiverStore;

    #[test]
    fn stats_tracks_keys_and_operations() {
        let mut store = RiverStore::new();

        assert_eq!(store.stats().keys, 0);
        assert_eq!(store.stats().operations, 0);

        store.set("name".to_string(), "Water".to_string());
        assert_eq!(store.get("name").map(String::as_str), Some("Water"));
        store.delete("name");

        let stats = store.stats();
        assert_eq!(stats.keys, 0);
        assert_eq!(stats.operations, 3);
    }

    #[test]
    fn stats_does_not_increment_operations() {
        let mut store = RiverStore::new();

        let first = store.stats();
        let second = store.stats();

        assert_eq!(first.operations, 0);
        assert_eq!(second.operations, 0);
    }

    #[test]
    fn get_removes_expired_keys() {
        let mut store = RiverStore::new();

        store.set_with_expiration("session".to_string(), "abc".to_string(), 0);

        assert_eq!(store.get("session"), None);
        assert_eq!(store.stats().keys, 0);
    }

    #[test]
    fn expire_returns_false_for_missing_key() {
        let mut store = RiverStore::new();

        assert!(!store.expire("missing", 10));
    }

    #[test]
    fn set_clears_existing_expiration() {
        let mut store = RiverStore::new();

        store.set_with_expiration("session".to_string(), "abc".to_string(), 0);
        store.set("session".to_string(), "fresh".to_string());

        assert_eq!(store.get("session").map(String::as_str), Some("fresh"));
    }
}
