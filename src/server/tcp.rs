use crate::commands::{CommandResponse, handle_input};
use crate::store::engine::RiverStore;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

pub const DEFAULT_ADDRESS: &str = "127.0.0.1:6379";

pub async fn start_server(store: Arc<Mutex<RiverStore>>, address: &str) -> std::io::Result<()> {
    let listener = TcpListener::bind(address).await?;
    println!("River DB server started on {address}");

    loop {
        let (stream, address) = listener.accept().await?;
        println!("[INFO] Client connected: {address}");

        let store = Arc::clone(&store);
        tokio::spawn(async move {
            if let Err(error) = handle_client(stream, store).await {
                eprintln!("[ERROR] Client error: {error}");
            }
            println!("[INFO] Client disconnected: {address}");
        });
    }
}

async fn handle_client(stream: TcpStream, store: Arc<Mutex<RiverStore>>) -> std::io::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        let response = {
            let mut store = store.lock().await;
            handle_input(&line, &mut store)
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
