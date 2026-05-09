mod commands;
mod persistence;
mod server;
mod store;

use persistence::storage::{self, DEFAULT_DB_PATH};
use std::path::PathBuf;
use std::sync::Arc;
use store::engine::RiverStore;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let address =
        std::env::var("RIVER_ADDR").unwrap_or_else(|_| server::tcp::DEFAULT_ADDRESS.to_string());
    let db_path = PathBuf::from(
        std::env::var("RIVER_DB_PATH").unwrap_or_else(|_| DEFAULT_DB_PATH.to_string()),
    );
    let store = match storage::load_from_disk(&db_path) {
        Ok(Some(store)) => {
            println!("River DB restored from {}", db_path.display());
            store
        }
        Ok(None) => {
            println!("River DB starting with empty store");
            RiverStore::new()
        }
        Err(error) => {
            eprintln!(
                "ERROR: Failed to load {}: {error}. Starting with empty store.",
                db_path.display()
            );
            RiverStore::new()
        }
    };
    let store = Arc::new(Mutex::new(store));

    match server::tcp::start_server(store, &address, db_path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
            eprintln!(
                "ERROR: {address} is already in use. Stop the process using that port or run with RIVER_ADDR=127.0.0.1:6380."
            );
            Err(error)
        }
        Err(error) => Err(error),
    }
}
