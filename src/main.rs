use river::auth::AuthConfig;
use river::cli::CliArgs;
use river::logging;
use river::persistence::storage::{self, DEFAULT_DB_PATH};
use river::store::engine::RiverStore;
use river::store::shared::ConcurrentStore;
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    logging::init_logging();
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let first = argv.first().map(String::as_str).unwrap_or("server");
    match first {
        "server" => {
            if argv.len() != 1 {
                eprintln!("Usage: river [server|cli [--host <host>] [--port <port>] [--raw]]");
                std::process::exit(2);
            }
            std::process::exit(run_server().await);
        }
        "cli" => {
            let args = match CliArgs::parse(&argv[1..]) {
                Ok(args) => args,
                Err(usage) => {
                    eprintln!("{usage}");
                    std::process::exit(2);
                }
            };
            std::process::exit(river::cli::run_repl(&args).await);
        }
        _ => {
            eprintln!("Usage: river [server|cli [--host <host>] [--port <port>] [--raw]]");
            std::process::exit(2);
        }
    }
}

/// Bare `cargo run` (no subcommand) keeps launching the server.
async fn run_server() -> i32 {
    let address = std::env::var("RIVER_ADDR")
        .unwrap_or_else(|_| river::server::tcp::DEFAULT_ADDRESS.to_string());
    let db_path = PathBuf::from(
        std::env::var("RIVER_DB_PATH").unwrap_or_else(|_| DEFAULT_DB_PATH.to_string()),
    );
    let persisted = match storage::load_from_disk(&db_path) {
        Ok(Some(mut store)) => {
            store.cleanup_expired_on_startup();
            tracing::info!("restored store from {}", db_path.display());
            store
        }
        Ok(None) => {
            tracing::info!("starting with empty store");
            RiverStore::new()
        }
        Err(error) => {
            tracing::error!(
                "failed to load {}: {error}; starting with empty store",
                db_path.display()
            );
            RiverStore::new()
        }
    };
    let shard_count = std::env::var("RIVER_SHARDS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get() * 4)
                .unwrap_or(8)
        });
    let store = Arc::new(ConcurrentStore::from_persisted(persisted, shard_count).await);
    let auth = AuthConfig::from_env();
    if auth.is_enabled() {
        tracing::info!("authentication enabled (RIVER_PASSWORD is set)");
    } else {
        tracing::info!("authentication disabled (localhost development mode)");
    }

    match river::server::tcp::start_server(store, &address, db_path, auth).await {
        Ok(()) => 0,
        Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
            tracing::error!(
                "{address} is already in use; stop that process or run with RIVER_ADDR=127.0.0.1:6380"
            );
            1
        }
        Err(error) => {
            tracing::error!("server failed: {error}");
            1
        }
    }
}
