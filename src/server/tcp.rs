use crate::auth::AuthConfig;
use crate::commands::{CommandResponse, handle_parts};
use crate::metrics::ActiveConnectionGuard;
use crate::protocol::frame::Frame;
use crate::protocol::resp::{self, DecodeResult};
use crate::store::shared::ConcurrentStore;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{self, Duration};

pub const DEFAULT_ADDRESS: &str = "127.0.0.1:2007";

pub async fn start_server(
    store: Arc<ConcurrentStore>,
    address: &str,
    db_path: PathBuf,
    auth: AuthConfig,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(address).await?;
    tracing::info!("River DB server started on {address}");
    tracing::info!("persistence file: {}", db_path.display());
    serve_on_listener(listener, store, db_path, auth).await
}

pub async fn serve_on_listener(
    listener: TcpListener,
    store: Arc<ConcurrentStore>,
    db_path: PathBuf,
    auth: AuthConfig,
) -> std::io::Result<()> {
    start_cleanup_worker(Arc::clone(&store), db_path.clone());

    loop {
        let (stream, address) = listener.accept().await?;
        tracing::info!("client connected: {address}");

        let store = Arc::clone(&store);
        let db_path = db_path.clone();
        let auth = auth.clone();
        tokio::spawn(async move {
            if let Err(error) = handle_client(stream, store, db_path, auth).await {
                tracing::error!("client error: {error}");
            }
            tracing::info!("client disconnected: {address}");
        });
    }
}

fn start_cleanup_worker(store: Arc<ConcurrentStore>, db_path: PathBuf) {
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(1));

        loop {
            interval.tick().await;
            let removed = store.cleanup_expired().await;
            store.metrics().record_cleanup_run();
            if removed > 0 {
                store.metrics().record_expired(removed);
                let snapshot = store.snapshot().await;
                if let Err(error) = crate::persistence::storage::save_to_disk(&snapshot, &db_path) {
                    store.metrics().record_persistence_failed();
                    tracing::error!("failed to persist expired key cleanup: {error}");
                } else {
                    store.metrics().record_persistence_saved();
                }
                tracing::info!("removed {removed} expired key(s)");
            }
        }
    });
}

async fn handle_client(
    stream: TcpStream,
    store: Arc<ConcurrentStore>,
    db_path: PathBuf,
    auth: AuthConfig,
) -> std::io::Result<()> {
    // Covers every exit path below (EOF, EXIT/QUIT, protocol error, I/O
    // error via `?`): dropping the guard always decrements the gauge.
    let _connection = ActiveConnectionGuard::open(store.metrics());
    // Session state: local to this connection, never shared, discarded on
    // disconnect. Open servers start authenticated (unchanged behavior).
    let mut authenticated = !auth.is_enabled();
    let (mut reader, mut writer) = stream.into_split();
    let mut buffer = Vec::new();
    let mut read_buffer = [0; 4096];

    loop {
        let read = reader.read(&mut read_buffer).await?;
        if read == 0 {
            if !buffer.is_empty() {
                let frame = Frame::Error("ERROR incomplete frame".to_string());
                writer.write_all(&resp::encode(&frame)).await?;
            }
            break;
        }

        buffer.extend_from_slice(&read_buffer[..read]);

        loop {
            let decoded = match resp::decode(&buffer) {
                Ok(DecodeResult::Complete(frame, consumed)) => {
                    buffer.drain(..consumed);
                    frame
                }
                Ok(DecodeResult::Incomplete) => break,
                Err(error) => {
                    store.metrics().record_protocol_error();
                    tracing::warn!("malformed RESP frame: {}", error.message());
                    let frame = Frame::Error(format!("ERROR {}", error.message()));
                    writer.write_all(&resp::encode(&frame)).await?;
                    buffer.clear();
                    break;
                }
            };

            let response = match resp::frame_to_parts(decoded) {
                Ok(Some(parts)) => {
                    if !authenticated && !pre_auth_allowed(&parts) {
                        store.metrics().record_auth_required();
                        tracing::warn!("rejected unauthenticated command");
                        CommandResponse::Error("ERROR authentication required".to_string())
                    } else {
                        let response =
                            handle_parts(&parts, &store, Some(&db_path), &auth).await;
                        if is_auth_attempt(&parts)
                            && matches!(response, CommandResponse::Simple(_))
                        {
                            authenticated = true;
                        }
                        response
                    }
                }
                Ok(None) => CommandResponse::Empty,
                Err(error) => {
                    store.metrics().record_protocol_error();
                    tracing::warn!("non-command frame: {}", error.message());
                    CommandResponse::Error(format!("ERROR {}", error.message()))
                }
            };

            match response {
                CommandResponse::Simple(message) => {
                    writer
                        .write_all(&resp::encode(&Frame::Simple(message)))
                        .await?;
                }
                CommandResponse::Integer(value) => {
                    writer
                        .write_all(&resp::encode(&Frame::Integer(value)))
                        .await?;
                }
                CommandResponse::Bulk(message) => {
                    writer
                        .write_all(&resp::encode(&Frame::Bulk(message)))
                        .await?;
                }
                CommandResponse::Null => {
                    writer.write_all(&resp::encode(&Frame::Null)).await?;
                }
                CommandResponse::Error(message) => {
                    writer
                        .write_all(&resp::encode(&Frame::Error(message)))
                        .await?;
                }
                CommandResponse::Empty => {}
                CommandResponse::Close => return Ok(()),
            }
        }
    }

    Ok(())
}

/// Command name for session decisions, without invoking the parser.
fn request_name(parts: &[String]) -> &str {
    parts.first().map(String::as_str).unwrap_or("")
}

/// Before authentication, only AUTH itself and clean disconnects are
/// allowed. Everything else — including PING, STATS, and HEALTH — requires
/// an authenticated session, so unauthenticated clients learn nothing about
/// the database.
fn pre_auth_allowed(parts: &[String]) -> bool {
    matches!(
        request_name(parts).to_ascii_uppercase().as_str(),
        "AUTH" | "QUIT" | "EXIT"
    )
}

fn is_auth_attempt(parts: &[String]) -> bool {
    request_name(parts).eq_ignore_ascii_case("AUTH")
}
