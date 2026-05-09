mod commands;
mod server;
mod store;

use std::sync::Arc;
use store::engine::RiverStore;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let store = Arc::new(Mutex::new(RiverStore::new()));
    let address =
        std::env::var("RIVER_ADDR").unwrap_or_else(|_| server::tcp::DEFAULT_ADDRESS.to_string());

    match server::tcp::start_server(store, &address).await {
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
