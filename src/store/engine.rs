use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct RiverStore {
    data: HashMap<String, String>,
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
            operations: 0,
        }
    }

    pub fn set(&mut self, key: String, value: String) {
        self.operations += 1;
        self.data.insert(key, value);
    }

    pub fn get(&mut self, key: &str) -> Option<&String> {
        self.operations += 1;
        self.data.get(key)
    }

    pub fn delete(&mut self, key: &str) {
        self.operations += 1;
        self.data.remove(key);
    }

    pub fn stats(&self) -> StoreStats {
        StoreStats {
            keys: self.data.len(),
            operations: self.operations,
        }
    }

    pub fn health(&self) -> StoreHealth {
        StoreHealth {
            status: "OK",
            keys: self.data.len(),
            operations: self.operations,
            uptime: 0,
        }
    }
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
        let store = RiverStore::new();

        let first = store.stats();
        let second = store.stats();

        assert_eq!(first.operations, 0);
        assert_eq!(second.operations, 0);
    }
}
