use crate::protocol::frame::Frame;
use crate::protocol::resp::{self, DecodeResult};
use std::fmt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Error connecting to or talking to the River server.
///
/// Display strings are human-facing; no internal chains leak to the CLI.
#[derive(Debug)]
pub enum ClientError {
    Connection(String),
    Protocol(String),
    Closed,
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connection(addr) => {
                write!(f, "Could not connect to River at {addr}")
            }
            Self::Protocol(msg) => write!(f, "Protocol error: {msg}"),
            Self::Closed => write!(f, "Connection closed by server"),
        }
    }
}

impl std::error::Error for ClientError {}

/// A persistent TCP connection to one River server.
///
/// Requests are encoded with the existing RESP encoder and responses are
/// decoded with the existing RESP decoder — the same framing `nc` clients
/// use. This type holds no database state.
#[derive(Debug)]
pub struct RiverClient {
    stream: TcpStream,
    read_buf: Vec<u8>,
    raw: bool,
}

impl RiverClient {
    pub async fn connect(host: &str, port: u16, raw: bool) -> Result<Self, ClientError> {
        let addr = format!("{host}:{port}");
        let stream = TcpStream::connect(&addr)
            .await
            .map_err(|_| ClientError::Connection(addr))?;
        Ok(Self {
            stream,
            read_buf: Vec::new(),
            raw,
        })
    }

    pub fn raw(&self) -> bool {
        self.raw
    }

    /// Authenticate this connection with a password. Returns `Ok(true)` on
    /// success, `Ok(false)` when the server has authentication disabled
    /// (nothing to do), and `Err` on wrong credentials or I/O failure.
    /// The password is sent once and never stored or printed.
    pub async fn authenticate(&mut self, password: &str) -> Result<bool, ClientError> {
        let parts = vec!["AUTH".to_string(), password.to_string()];
        match self.execute(&parts).await? {
            Frame::Simple(_) => Ok(true),
            Frame::Error(message) if message == "ERROR authentication not required" => Ok(false),
            Frame::Error(message) => Err(ClientError::Protocol(message)),
            _ => Err(ClientError::Protocol("unexpected AUTH response".to_string())),
        }
    }

    /// Send one command (already-split parts) and return the decoded frame.
    pub async fn execute(&mut self, parts: &[String]) -> Result<Frame, ClientError> {
        let request = Frame::Array(parts.iter().map(|p| Frame::Bulk(p.clone())).collect());
        let bytes = resp::encode(&request);
        if self.raw {
            eprintln!("[raw request] {}", escape_bytes(&bytes));
        }
        self.stream
            .write_all(&bytes)
            .await
            .map_err(|e| ClientError::Protocol(e.to_string()))?;
        self.read_frame().await
    }

    async fn read_frame(&mut self) -> Result<Frame, ClientError> {
        let mut tmp = [0u8; 4096];
        loop {
            match resp::decode(&self.read_buf).map_err(|e| ClientError::Protocol(e.message().into()))? {
                DecodeResult::Complete(frame, consumed) => {
                    self.read_buf.drain(..consumed);
                    if self.raw {
                        eprintln!("[raw response] {}", escape_bytes(&resp::encode(&frame)));
                    }
                    return Ok(frame);
                }
                DecodeResult::Incomplete => {}
            }
            let n = self
                .stream
                .read(&mut tmp)
                .await
                .map_err(|e| ClientError::Protocol(e.to_string()))?;
            if n == 0 {
                return Err(ClientError::Closed);
            }
            self.read_buf.extend_from_slice(&tmp[..n]);
        }
    }
}

fn escape_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .flat_map(|b| std::ascii::escape_default(*b))
        .map(|b| b as char)
        .collect()
}
