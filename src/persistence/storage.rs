use crate::store::engine::RiverStore;
use std::fmt;
use std::path::Path;

pub const DEFAULT_DB_PATH: &str = "river.db";

#[derive(Debug)]
pub enum PersistenceError {
    Io(std::io::Error),
    Encode(Box<bincode::ErrorKind>),
}

impl fmt::Display for PersistenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "disk I/O failed: {error}"),
            Self::Encode(error) => write!(formatter, "database serialization failed: {error}"),
        }
    }
}

impl std::error::Error for PersistenceError {}

impl From<std::io::Error> for PersistenceError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<Box<bincode::ErrorKind>> for PersistenceError {
    fn from(error: Box<bincode::ErrorKind>) -> Self {
        Self::Encode(error)
    }
}

pub fn save_to_disk(store: &RiverStore, path: impl AsRef<Path>) -> Result<(), PersistenceError> {
    let path = path.as_ref();
    let encoded = bincode::serialize(store)?;
    let temp_path = path.with_extension("db.tmp");

    std::fs::write(&temp_path, encoded)?;
    std::fs::rename(temp_path, path)?;

    Ok(())
}

pub fn load_from_disk(path: impl AsRef<Path>) -> Result<Option<RiverStore>, PersistenceError> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(None);
    }

    let bytes = std::fs::read(path)?;
    let store = bincode::deserialize(&bytes)?;

    Ok(Some(store))
}

#[cfg(test)]
mod tests {
    use super::{load_from_disk, save_to_disk};
    use crate::store::engine::RiverStore;
    use std::path::PathBuf;

    fn test_db_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("river-{name}-{}.db", std::process::id()))
    }

    #[test]
    fn missing_file_loads_as_empty_option() {
        let path = test_db_path("missing");
        let _ = std::fs::remove_file(&path);

        let loaded = load_from_disk(&path).expect("missing file should not be an error");

        assert!(loaded.is_none());
    }

    #[test]
    fn saves_and_loads_store_data() {
        let path = test_db_path("round-trip");
        let _ = std::fs::remove_file(&path);

        let mut store = RiverStore::new();
        store.set("name".to_string(), "Water".to_string());
        save_to_disk(&store, &path).expect("store should save");

        let mut loaded = load_from_disk(&path)
            .expect("store should load")
            .expect("database file should contain a store");

        assert_eq!(loaded.get("name").map(String::as_str), Some("Water"));
        assert_eq!(loaded.stats().keys, 1);
        assert_eq!(loaded.stats().operations, 1);

        let _ = std::fs::remove_file(&path);
    }
}
