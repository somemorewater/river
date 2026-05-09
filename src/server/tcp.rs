use crate::commands::{CommandResponse, handle_input_with_persistence};
use crate::store::engine::RiverStore;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
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
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        let response = {
            let mut store = store.lock().await;
            handle_input_with_persistence(&line, &mut store, Some(&db_path))
        };

        match response {
            CommandResponse::Message(message) => {
                writer.write_all(message.as_bytes()).await?;
                writer.write_all(b"\n").await?;
            }
            CommandResponse::Empty => {}
            CommandResponse::Close => break,
        }
    }

    Ok(())
}
