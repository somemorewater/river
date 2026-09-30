//! TCP/RESP end-to-end tests for the real server boundary:
//! TCP client -> tcp.rs -> resp decode -> parser -> ConcurrentStore -> resp encode.
//! Each test boots an isolated server on 127.0.0.1:0 (never the production 2007).

use river::protocol::frame::Frame;
use river::protocol::resp::{self, DecodeResult};
use river::store::shared::ConcurrentStore;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TestServer {
    addr: SocketAddr,
    db_path: PathBuf,
    tmp_dir: PathBuf,
    handle: tokio::task::JoinHandle<std::io::Result<()>>,
}

impl TestServer {
    async fn start() -> Self {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let tmp_dir = std::env::temp_dir().join(format!(
            "river-tcp-test-{}-{}-{}",
            std::process::id(),
            id,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&tmp_dir).expect("create temp dir");
        let db_path = tmp_dir.join("river.db");

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local addr");
        let store = Arc::new(ConcurrentStore::new(8));
        let db = db_path.clone();
        let handle = tokio::spawn(async move {
            river::server::tcp::serve_on_listener(listener, store, db).await
        });

        // Wait until the listener accepts (no arbitrary long sleep).
        let mut last_err = None;
        for _ in 0..50 {
            match TcpStream::connect(addr).await {
                Ok(_) => {
                    last_err = None;
                    break;
                }
                Err(e) => {
                    last_err = Some(e);
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            }
        }
        if let Some(e) = last_err {
            panic!("test server never became connectable: {e}");
        }

        Self {
            addr,
            db_path,
            tmp_dir,
            handle,
        }
    }

    /// Restart a server on the SAME persistence file (new ephemeral port),
    /// replicating src/main.rs startup: load -> cleanup expired -> serve.
    async fn restart_on_same_db(&self) -> RestartedServer {
        use river::persistence::storage;
        use river::store::engine::RiverStore;

        let persisted = match storage::load_from_disk(&self.db_path).expect("load db file") {
            Some(mut store) => {
                store.cleanup_expired_on_startup();
                store
            }
            None => RiverStore::new(),
        };
        let store = Arc::new(ConcurrentStore::from_persisted(persisted, 8).await);
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind restart port");
        let addr = listener.local_addr().expect("local addr");
        let db = self.db_path.clone();
        let handle = tokio::spawn(async move {
            river::server::tcp::serve_on_listener(listener, store, db).await
        });
        // readiness probe
        for _ in 0..50 {
            if TcpStream::connect(addr).await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        RestartedServer { addr, handle }
    }

    fn stop(&self) {
        self.handle.abort();
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.handle.abort();
        let _ = std::fs::remove_dir_all(&self.tmp_dir);
    }
}

struct RestartedServer {
    addr: SocketAddr,
    handle: tokio::task::JoinHandle<std::io::Result<()>>,
}

impl Drop for RestartedServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

struct RespConn {
    stream: TcpStream,
    buf: Vec<u8>,
}

fn array(parts: &[&str]) -> Frame {
    Frame::Array(
        parts
            .iter()
            .map(|s| Frame::Bulk(s.to_string()))
            .collect(),
    )
}

impl RespConn {
    async fn connect(addr: SocketAddr) -> Self {
        let stream = tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(addr))
            .await
            .expect("connect timeout")
            .expect("connect");
        Self {
            stream,
            buf: Vec::new(),
        }
    }

    async fn send_frame(&mut self, frame: &Frame) {
        let bytes = resp::encode(frame);
        self.stream
            .write_all(&bytes)
            .await
            .expect("write frame");
    }

    async fn cmd(&mut self, parts: &[&str]) -> Frame {
        self.send_frame(&array(parts)).await;
        self.read_frame().await
    }

    async fn read_frame(&mut self) -> Frame {
        let mut tmp = [0u8; 4096];
        loop {
            match resp::decode(&self.buf).expect("client decode must not fail") {
                DecodeResult::Complete(frame, consumed) => {
                    self.buf.drain(..consumed);
                    return frame;
                }
                DecodeResult::Incomplete => {}
            }
            let n = tokio::time::timeout(Duration::from_secs(5), self.stream.read(&mut tmp))
                .await
                .expect("read timeout")
                .expect("read");
            assert!(n > 0, "server closed connection unexpectedly");
            self.buf.extend_from_slice(&tmp[..n]);
        }
    }
}

// ---------- connection + basic commands ----------

#[tokio::test]
async fn ping_roundtrip() {
    let server = TestServer::start().await;
    let mut conn = RespConn::connect(server.addr).await;
    let response = conn.cmd(&["PING"]).await;
    assert_eq!(response, Frame::Simple("PONG".to_string()));
}

#[tokio::test]
async fn set_get_roundtrip_with_spaces() {
    let server = TestServer::start().await;
    let mut conn = RespConn::connect(server.addr).await;

    assert_eq!(
        conn.cmd(&["SET", "name", "Water"]).await,
        Frame::Simple("OK".to_string())
    );
    assert_eq!(
        conn.cmd(&["GET", "name"]).await,
        Frame::Bulk("Water".to_string())
    );
    // Bulk strings carry spaces without tokenizing (no plain-text path).
    assert_eq!(
        conn.cmd(&["SET", "greeting", "hello river world"]).await,
        Frame::Simple("OK".to_string())
    );
    assert_eq!(
        conn.cmd(&["GET", "greeting"]).await,
        Frame::Bulk("hello river world".to_string())
    );
}

#[tokio::test]
async fn missing_key_returns_null() {
    let server = TestServer::start().await;
    let mut conn = RespConn::connect(server.addr).await;
    let response = conn.cmd(&["GET", "no-such-key"]).await;
    assert_eq!(response, Frame::Null);
    assert_eq!(resp::encode(&response), b"$-1\r\n");
}

#[tokio::test]
async fn del_cycle_and_missing_del() {
    let server = TestServer::start().await;
    let mut conn = RespConn::connect(server.addr).await;

    assert_eq!(
        conn.cmd(&["SET", "k", "v"]).await,
        Frame::Simple("OK".to_string())
    );
    assert_eq!(
        conn.cmd(&["DEL", "k"]).await,
        Frame::Simple("OK".to_string())
    );
    assert_eq!(conn.cmd(&["GET", "k"]).await, Frame::Null);
    // Deleting a missing key is still OK and must not crash the server.
    assert_eq!(
        conn.cmd(&["DEL", "k"]).await,
        Frame::Simple("OK".to_string())
    );
    assert_eq!(conn.cmd(&["PING"]).await, Frame::Simple("PONG".to_string()));
}

#[tokio::test]
async fn sequential_commands_on_one_connection() {
    let server = TestServer::start().await;
    let mut conn = RespConn::connect(server.addr).await;

    let cases: &[(&[&str], Frame)] = &[
        (&["SET", "a", "1"], Frame::Simple("OK".to_string())),
        (&["SET", "b", "2"], Frame::Simple("OK".to_string())),
        (&["GET", "a"], Frame::Bulk("1".to_string())),
        (&["GET", "b"], Frame::Bulk("2".to_string())),
        (&["DEL", "a"], Frame::Simple("OK".to_string())),
        (&["GET", "a"], Frame::Null),
    ];
    for (parts, expected) in cases {
        assert_eq!(conn.cmd(parts).await, *expected, "for {parts:?}");
    }
}

// ---------- pipelining + partial reads ----------

#[tokio::test]
async fn pipelined_frames_all_execute_in_order() {
    let server = TestServer::start().await;
    let mut conn = RespConn::connect(server.addr).await;

    // Encode four commands and write them as one blob (tcp.rs drains in order).
    let mut blob = Vec::new();
    for parts in [
        vec!["SET", "pa", "1"],
        vec!["SET", "pb", "2"],
        vec!["GET", "pa"],
        vec!["GET", "pb"],
    ] {
        let frame = Frame::Array(parts.into_iter().map(|s| Frame::Bulk(s.into())).collect());
        blob.extend_from_slice(&resp::encode(&frame));
    }
    conn.stream.write_all(&blob).await.expect("write blob");

    let expected = [
        Frame::Simple("OK".to_string()),
        Frame::Simple("OK".to_string()),
        Frame::Bulk("1".to_string()),
        Frame::Bulk("2".to_string()),
    ];
    for want in expected {
        assert_eq!(conn.read_frame().await, want);
    }
    // Connection remains usable.
    assert_eq!(conn.cmd(&["PING"]).await, Frame::Simple("PONG".to_string()));
}

#[tokio::test]
async fn fragmented_frame_is_reconstructed() {
    let server = TestServer::start().await;
    let mut conn = RespConn::connect(server.addr).await;

    // Split one SET frame into two TCP writes to exercise Incomplete handling.
    let full = resp::encode(&array(&["SET", "frag", "value"]));
    let split = full.len() / 2;
    conn.stream
        .write_all(&full[..split])
        .await
        .expect("write part 1");
    tokio::time::sleep(Duration::from_millis(50)).await;
    conn.stream
        .write_all(&full[split..])
        .await
        .expect("write part 2");

    assert_eq!(
        conn.read_frame().await,
        Frame::Simple("OK".to_string())
    );
    assert_eq!(
        conn.cmd(&["GET", "frag"]).await,
        Frame::Bulk("value".to_string())
    );
}

// ---------- errors ----------

#[tokio::test]
async fn malformed_resp_returns_error_and_keeps_connection() {
    let server = TestServer::start().await;
    let mut conn = RespConn::connect(server.addr).await;

    // 'H' is not a valid RESP type marker -> server decodes Err, replies Error, clears buffer.
    conn.stream.write_all(b"HELLO\r\n").await.expect("write");
    let response = conn.read_frame().await;
    match response {
        Frame::Error(msg) => assert!(msg.starts_with("ERROR "), "got {msg:?}"),
        other => panic!("expected error frame, got {other:?}"),
    }
    // Server must still serve this connection.
    assert_eq!(conn.cmd(&["PING"]).await, Frame::Simple("PONG".to_string()));
}

#[tokio::test]
async fn unknown_command_and_bad_arity_do_not_kill_server() {
    let server = TestServer::start().await;
    let mut conn = RespConn::connect(server.addr).await;

    assert_eq!(
        conn.cmd(&["UNKNOWN", "something"]).await,
        Frame::Error("ERROR unknown command".to_string())
    );
    assert_eq!(
        conn.cmd(&["GET"]).await,
        Frame::Error("ERROR invalid syntax".to_string())
    );
    assert_eq!(
        conn.cmd(&["SET", "only-key"]).await,
        Frame::Error("ERROR invalid syntax".to_string())
    );
    assert_eq!(conn.cmd(&["PING"]).await, Frame::Simple("PONG".to_string()));
}

// ---------- TTL over TCP ----------

#[tokio::test]
async fn setex_value_expires_over_tcp() {
    let server = TestServer::start().await;
    let mut conn = RespConn::connect(server.addr).await;

    assert_eq!(
        conn.cmd(&["SETEX", "temp", "1", "value"]).await,
        Frame::Simple("OK".to_string())
    );
    assert_eq!(
        conn.cmd(&["GET", "temp"]).await,
        Frame::Bulk("value".to_string())
    );
    tokio::time::sleep(Duration::from_millis(1600)).await;
    assert_eq!(conn.cmd(&["GET", "temp"]).await, Frame::Null);
}

#[tokio::test]
async fn expire_attaches_ttl_over_tcp() {
    let server = TestServer::start().await;
    let mut conn = RespConn::connect(server.addr).await;

    assert_eq!(
        conn.cmd(&["EXPIRE", "missing", "60"]).await,
        Frame::Integer(0)
    );
    assert_eq!(
        conn.cmd(&["SET", "perm", "value"]).await,
        Frame::Simple("OK".to_string())
    );
    assert_eq!(
        conn.cmd(&["EXPIRE", "perm", "1"]).await,
        Frame::Integer(1)
    );
    assert_eq!(
        conn.cmd(&["GET", "perm"]).await,
        Frame::Bulk("value".to_string())
    );
    tokio::time::sleep(Duration::from_millis(1600)).await;
    assert_eq!(conn.cmd(&["GET", "perm"]).await, Frame::Null);
}

// ---------- persistence restart ----------

#[tokio::test]
async fn persistence_restores_value_after_restart() {
    let server = TestServer::start().await;
    let mut conn = RespConn::connect(server.addr).await;
    assert_eq!(
        conn.cmd(&["SET", "persisted", "hello"]).await,
        Frame::Simple("OK".to_string())
    );
    // SET awaits persist_snapshot before replying, but poll briefly for the file.
    let mut found = false;
    for _ in 0..50 {
        if server.db_path.exists() {
            found = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(found, "river.db was not created after SET");
    drop(conn);
    server.stop();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let restarted = server.restart_on_same_db().await;
    let mut conn2 = RespConn::connect(restarted.addr).await;
    assert_eq!(
        conn2.cmd(&["GET", "persisted"]).await,
        Frame::Bulk("hello".to_string())
    );
}

// ---------- concurrency + disconnect ----------

#[tokio::test]
async fn multiple_clients_share_state() {
    let server = TestServer::start().await;

    let mut setup = RespConn::connect(server.addr).await;
    assert_eq!(
        setup.cmd(&["SET", "shared", "42"]).await,
        Frame::Simple("OK".to_string())
    );
    drop(setup);

    let addr = server.addr;
    let mut tasks = Vec::new();
    for i in 0..4u32 {
        tasks.push(tokio::spawn(async move {
            let mut conn = RespConn::connect(addr).await;
            // Every client sees shared state.
            assert_eq!(
                conn.cmd(&["GET", "shared"]).await,
                Frame::Bulk("42".to_string())
            );
            // Each client writes its own key.
            let key = format!("client-{i}");
            assert_eq!(
                conn.cmd(&["SET", &key, "x"]).await,
                Frame::Simple("OK".to_string())
            );
            assert_eq!(
                conn.cmd(&["GET", &key]).await,
                Frame::Bulk("x".to_string())
            );
        }));
    }
    for task in tasks {
        task.await.expect("client task");
    }

    // All per-client keys are visible from a fresh connection.
    let mut check = RespConn::connect(server.addr).await;
    for i in 0..4u32 {
        assert_eq!(
            check.cmd(&["GET", &format!("client-{i}")]).await,
            Frame::Bulk("x".to_string())
        );
    }
}

#[tokio::test]
async fn exit_closes_client_but_server_survives() {
    let server = TestServer::start().await;
    let mut conn = RespConn::connect(server.addr).await;
    conn.send_frame(&array(&["EXIT"])).await;
    // EXIT produces no response bytes; the server closes this connection (EOF).
    let mut tmp = [0u8; 64];
    let n = tokio::time::timeout(Duration::from_secs(5), conn.stream.read(&mut tmp))
        .await
        .expect("read timeout")
        .expect("read");
    assert_eq!(n, 0, "EXIT should close the connection without a reply");

    // Server remains alive for new clients.
    let mut conn2 = RespConn::connect(server.addr).await;
    assert_eq!(conn2.cmd(&["PING"]).await, Frame::Simple("PONG".to_string()));

    // Plain disconnect also leaves the server usable.
    drop(conn2);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let mut conn3 = RespConn::connect(server.addr).await;
    assert_eq!(conn3.cmd(&["PING"]).await, Frame::Simple("PONG".to_string()));
}
