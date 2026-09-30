//! CLI client tests: `RiverClient` against a real River server on an
//! ephemeral port. Covers request/response framing only — REPL parsing and
//! formatting are unit-tested in `src/cli/repl.rs`.

use river::cli::RiverClient;
use river::protocol::frame::Frame;
use river::store::shared::ConcurrentStore;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};

async fn start_server() -> (SocketAddr, tokio::task::JoinHandle<std::io::Result<()>>) {
    let dir = std::env::temp_dir().join(format!(
        "river-cli-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let store = Arc::new(ConcurrentStore::new(8));
    let handle = tokio::spawn(async move {
        river::server::tcp::serve_on_listener(listener, store, dir.join("river.db"), river::auth::AuthConfig::disabled()).await
    });
    for _ in 0..50 {
        if TcpStream::connect(addr).await.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    (addr, handle)
}

fn parts(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

#[tokio::test]
async fn client_ping_and_set_get() {
    let (addr, handle) = start_server().await;
    let mut client = RiverClient::connect("127.0.0.1", addr.port(), false)
        .await
        .expect("connect");
    assert_eq!(
        client.execute(&parts(&["PING"])).await.expect("ping"),
        Frame::Simple("PONG".to_string())
    );
    assert_eq!(
        client
            .execute(&parts(&["SET", "name", "Water"]))
            .await
            .expect("set"),
        Frame::Simple("OK".to_string())
    );
    assert_eq!(
        client.execute(&parts(&["GET", "name"])).await.expect("get"),
        Frame::Bulk("Water".to_string())
    );
    handle.abort();
}

#[tokio::test]
async fn client_sees_null_and_errors() {
    let (addr, handle) = start_server().await;
    let mut client = RiverClient::connect("127.0.0.1", addr.port(), false)
        .await
        .expect("connect");
    assert_eq!(
        client
            .execute(&parts(&["GET", "missing"]))
            .await
            .expect("get"),
        Frame::Null
    );
    assert_eq!(
        client
            .execute(&parts(&["NOPE"]))
            .await
            .expect("unknown"),
        Frame::Error("ERROR unknown command".to_string())
    );
    handle.abort();
}

#[tokio::test]
async fn client_connection_failure_is_friendly() {
    // Port 1 is (practically) never listening: must be a clean message.
    let err = RiverClient::connect("127.0.0.1", 1, false)
        .await
        .expect_err("connect should fail");
    assert_eq!(
        err.to_string(),
        "Could not connect to River at 127.0.0.1:1"
    );
}

async fn start_authed_server(
    password: &str,
) -> (
    std::net::SocketAddr,
    tokio::task::JoinHandle<std::io::Result<()>>,
) {
    use river::store::shared::ConcurrentStore;
    use std::sync::Arc;
    let dir = std::env::temp_dir().join(format!(
        "river-cli-auth-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let store = Arc::new(ConcurrentStore::new(8));
    let auth = river::auth::AuthConfig::with_password(password);
    let handle = tokio::spawn(async move {
        river::server::tcp::serve_on_listener(listener, store, dir.join("river.db"), auth).await
    });
    for _ in 0..50 {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    (addr, handle)
}

#[tokio::test]
async fn client_authenticate_flow() {
    let (addr, handle) = start_authed_server("s3cr3t").await;
    let mut client = RiverClient::connect("127.0.0.1", addr.port(), false)
        .await
        .expect("connect");

    // Wrong password fails without authenticating.
    assert!(client.authenticate("wrong").await.is_err());

    // Right password succeeds and unlocks commands.
    assert_eq!(client.authenticate("s3cr3t").await.expect("auth"), true);
    assert_eq!(
        client
            .execute(&parts(&["SET", "k", "v"]))
            .await
            .expect("set"),
        Frame::Simple("OK".to_string())
    );
    handle.abort();
}

#[tokio::test]
async fn client_authenticate_reports_open_server() {
    let (addr, handle) = start_server().await;
    let mut client = RiverClient::connect("127.0.0.1", addr.port(), false)
        .await
        .expect("connect");
    // No password required: reports false so the CLI just proceeds.
    assert_eq!(client.authenticate("anything").await.expect("auth"), false);
    handle.abort();
}
