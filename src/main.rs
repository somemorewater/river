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
            if argv.len() > 3 || (argv.len() > 1 && argv[1] != "--http-addr") || argv.len() == 2 {
                eprintln!(
                    "Usage: river server [--http-addr <address>] | river cli [--host <host>] [--port <port>] [--raw]"
                );
                std::process::exit(2);
            }
            let http_addr = if argv.len() == 3 {
                Some(argv[2].clone())
            } else {
                None
            };
            std::process::exit(run_server(http_addr).await);
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
async fn run_server(http_override: Option<String>) -> i32 {
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
    let http_address = http_override
        .or_else(|| std::env::var("RIVER_HTTP_ADDR").ok())
        .unwrap_or_else(|| river::server::web::DEFAULT_ADDRESS.to_string());
    if auth.is_enabled() {
        tracing::info!("authentication enabled (RIVER_PASSWORD is set)");
    } else {
        tracing::info!("authentication disabled (localhost development mode)");
    }

    let web_listener = match tokio::net::TcpListener::bind(&http_address).await {
        Ok(listener) => listener,
        Err(error) => {
            tracing::error!("web client failed to bind {http_address}: {error}");
            return 1;
        }
    };
    let tcp_listener = match tokio::net::TcpListener::bind(&address).await {
        Ok(listener) => listener,
        Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
            tracing::error!(
                "{address} is already in use; stop that process or run with RIVER_ADDR=127.0.0.1:6380"
            );
            return 1;
        }
        Err(error) => {
            tracing::error!("server failed to bind {address}: {error}");
            return 1;
        }
    };
    let web_actual = web_listener
        .local_addr()
        .expect("bound listener has local address");
    let tcp_store = Arc::clone(&store);
    let tcp_db_path = db_path.clone();
    let tcp_auth = auth.clone();
    let web_store = Arc::clone(&store);
    let web_db_path = db_path.clone();
    let web_auth = auth.clone();
    let web_task = tokio::spawn(async move {
        river::server::web::serve_on_listener(web_listener, web_store, web_db_path, web_auth).await
    });
    tracing::info!("web client available at http://{web_actual}");
    match river::server::tcp::serve_bound_listener(tcp_listener, tcp_store, tcp_db_path, tcp_auth)
        .await
    {
        Ok(()) => 0,
        Err(error) => {
            web_task.abort();
            tracing::error!("server failed: {error}");
            1
        }
    }
}
