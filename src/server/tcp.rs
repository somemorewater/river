use crate::commands::{CommandResponse, handle_parts};
use crate::protocol::frame::Frame;
use crate::protocol::resp::{self, DecodeResult};
use crate::store::engine::RiverStore;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

pub const DEFAULT_ADDRESS: &str = "127.0.0.1:6379";

pub async fn start_server(
    store: Arc<Mutex<RiverStore>>,
    address: &str,
    db_path: PathBuf,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(address).await?;
    println!("River DB server started on {address}");
    println!("River DB persistence file: {}", db_path.display());

    loop {
        let (stream, address) = listener.accept().await?;
        println!("[INFO] Client connected: {address}");

        let store = Arc::clone(&store);
        let db_path = db_path.clone();
        tokio::spawn(async move {
            if let Err(error) = handle_client(stream, store, db_path).await {
                eprintln!("[ERROR] Client error: {error}");
            }
            println!("[INFO] Client disconnected: {address}");
        });
    }
}

async fn handle_client(
    stream: TcpStream,
    store: Arc<Mutex<RiverStore>>,
    db_path: PathBuf,
) -> std::io::Result<()> {
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
                    let frame = Frame::Error(format!("ERROR {}", error.message()));
                    writer.write_all(&resp::encode(&frame)).await?;
                    buffer.clear();
                    break;
                }
            };

            let response = match resp::frame_to_parts(decoded) {
                Ok(Some(parts)) => {
                    let mut store = store.lock().await;
                    handle_parts(&parts, &mut store, Some(&db_path))
                }
                Ok(None) => CommandResponse::Empty,
                Err(error) => CommandResponse::Error(format!("ERROR {}", error.message())),
            };

            match response {
                CommandResponse::Simple(message) => {
                    writer
                        .write_all(&resp::encode(&Frame::Simple(message)))
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
