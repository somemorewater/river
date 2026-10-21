use crate::auth::AuthConfig;
use crate::commands::parser::parse_parts;
use crate::commands::{CommandResponse, handle_parts};
use crate::store::shared::ConcurrentStore;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{Duration, timeout};

pub const DEFAULT_ADDRESS: &str = "127.0.0.1:8080";
const MAX_REQUEST_BYTES: usize = 16 * 1024;

#[derive(Deserialize)]
struct ApiRequest {
    command: String,
    #[serde(default)]
    password: String,
}

#[derive(Serialize)]
struct ApiResponse {
    ok: bool,
    response: Option<String>,
    error: Option<String>,
    kind: &'static str,
}

pub async fn serve_on_listener(
    listener: TcpListener,
    store: Arc<ConcurrentStore>,
    db_path: PathBuf,
    auth: AuthConfig,
) -> std::io::Result<()> {
    loop {
        let (stream, _) = listener.accept().await?;
        let store = Arc::clone(&store);
        let db_path = db_path.clone();
        let auth = auth.clone();
        tokio::spawn(async move {
            if let Err(error) = handle_connection(stream, store, db_path, auth).await {
                tracing::debug!("web client connection error: {error}");
            }
        });
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    store: Arc<ConcurrentStore>,
    db_path: PathBuf,
    auth: AuthConfig,
) -> std::io::Result<()> {
    let mut request = Vec::with_capacity(1024);
    let mut buf = [0u8; 2048];
    let header_end;
    loop {
        let n = match timeout(Duration::from_secs(5), stream.read(&mut buf)).await {
            Ok(result) => result?,
            Err(_) => return Ok(()),
        };
        if n == 0 {
            return Ok(());
        }
        request.extend_from_slice(&buf[..n]);
        if request.len() > MAX_REQUEST_BYTES {
            return respond(
                &mut stream,
                413,
                "application/json",
                br#"{"ok":false,"error":"request too large"}"#,
            )
            .await;
        }
        if let Some(pos) = request.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end = pos + 4;
            break;
        }
    }
    let header = std::str::from_utf8(&request[..header_end]).unwrap_or("");
    let mut lines = header.split("\r\n");
    let first = lines.next().unwrap_or("");
    let mut first_parts = first.split_whitespace();
    let method = first_parts.next().unwrap_or("");
    let path = first_parts.next().unwrap_or("");
    if method == "GET" && path == "/" {
        return respond(
            &mut stream,
            200,
            "text/html; charset=utf-8",
            include_bytes!("../../web/index.html"),
        )
        .await;
    }
    let (asset, mime) = match path {
        "/app.css" => (
            Some(&include_bytes!("../../web/app.css")[..]),
            "text/css; charset=utf-8",
        ),
        "/app.js" => (
            Some(&include_bytes!("../../web/app.js")[..]),
            "text/javascript; charset=utf-8",
        ),
        "/docs.json" => (
            Some(&include_bytes!("../../web/docs.json")[..]),
            "application/json",
        ),
        _ => (None, "application/json"),
    };
    if method == "GET"
        && let Some(asset) = asset
    {
        return respond(&mut stream, 200, mime, asset).await;
    }
    if method != "POST" || path != "/api/command" {
        return respond(
            &mut stream,
            404,
            "application/json",
            br#"{"ok":false,"error":"not found"}"#,
        )
        .await;
    }
    let mut content_length = None;
    let mut json_content_type = false;
    for line in lines {
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-type")
        {
            json_content_type = value
                .split(';')
                .next()
                .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"));
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            content_length = value.trim().parse::<usize>().ok();
        }
    }
    if !json_content_type {
        return respond(
            &mut stream,
            400,
            "application/json",
            br#"{"ok":false,"error":"content type must be application/json"}"#,
        )
        .await;
    }
    let Some(content_length) = content_length.filter(|n| *n <= MAX_REQUEST_BYTES) else {
        return respond(
            &mut stream,
            400,
            "application/json",
            br#"{"ok":false,"error":"invalid content length"}"#,
        )
        .await;
    };
    while request.len() - header_end < content_length {
        let n = match timeout(Duration::from_secs(5), stream.read(&mut buf)).await {
            Ok(result) => result?,
            Err(_) => {
                return respond(
                    &mut stream,
                    400,
                    "application/json",
                    br#"{"ok":false,"error":"request body timed out"}"#,
                )
                .await;
            }
        };
        if n == 0 {
            return respond(
                &mut stream,
                400,
                "application/json",
                br#"{"ok":false,"error":"incomplete request body"}"#,
            )
            .await;
        }
        request.extend_from_slice(&buf[..n]);
        if request.len() > MAX_REQUEST_BYTES {
            return respond(
                &mut stream,
                413,
                "application/json",
                br#"{"ok":false,"error":"request too large"}"#,
            )
            .await;
        }
    }
    let body = &request[header_end..header_end + content_length];
    let input: ApiRequest = match serde_json::from_slice(body) {
        Ok(input) => input,
        Err(_) => {
            return respond(
                &mut stream,
                400,
                "application/json",
                br#"{"ok":false,"error":"invalid request JSON"}"#,
            )
            .await;
        }
    };
    let parts = match tokenize(&input.command) {
        Some(parts) if !parts.is_empty() => parts,
        _ => {
            return json_response(
                &mut stream,
                400,
                ApiResponse {
                    ok: false,
                    response: None,
                    error: Some("Enter a command and its arguments".into()),
                    kind: "error",
                },
            )
            .await;
        }
    };
    if auth.is_enabled() && !auth.verify(&input.password) {
        store.metrics().record_auth_required();
        return json_response(
            &mut stream,
            401,
            ApiResponse {
                ok: false,
                response: None,
                error: Some("Authentication required or password incorrect".into()),
                kind: "error",
            },
        )
        .await;
    }
    let parsed = match parse_parts(&parts) {
        Ok(Some(command)) => command,
        Ok(None) => {
            return json_response(
                &mut stream,
                400,
                ApiResponse {
                    ok: false,
                    response: None,
                    error: Some("Empty command".into()),
                    kind: "error",
                },
            )
            .await;
        }
        Err(_) => {
            let error = handle_parts(&parts, &store, Some(&db_path), &auth).await;
            return command_response(&mut stream, error).await;
        }
    };
    if matches!(
        parsed,
        crate::commands::parser::Command::Auth { .. } | crate::commands::parser::Command::Exit
    ) {
        return json_response(
            &mut stream,
            400,
            ApiResponse {
                ok: false,
                response: None,
                error: Some("AUTH and EXIT are not available over HTTP".into()),
                kind: "error",
            },
        )
        .await;
    }
    let response = handle_parts(&parts, &store, Some(&db_path), &auth).await;
    command_response(&mut stream, response).await
}

// A shell-like tokenizer supports quoted values and backslash escapes while
// leaving command interpretation and validation to River's shared parser.
fn tokenize(line: &str) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut escaped = false;
    let mut active = false;
    for c in line.chars() {
        if escaped {
            word.push(c);
            escaped = false;
            active = true;
            continue;
        }
        if c == '\\' && quote != Some('\'') {
            escaped = true;
            active = true;
            continue;
        }
        if let Some(q) = quote {
            if c == q {
                quote = None;
            } else {
                word.push(c);
            }
            active = true;
            continue;
        }
        if c == '"' || c == '\'' {
            quote = Some(c);
            active = true;
        } else if c.is_whitespace() {
            if active {
                out.push(std::mem::take(&mut word));
                active = false;
            }
        } else {
            word.push(c);
            active = true;
        }
    }
    if escaped || quote.is_some() {
        return None;
    }
    if active {
        out.push(word);
    }
    Some(out)
}

async fn command_response(
    stream: &mut TcpStream,
    response: CommandResponse,
) -> std::io::Result<()> {
    let (ok, response, error, kind, status) = match response {
        CommandResponse::Simple(v) | CommandResponse::Bulk(v) => {
            (true, Some(v), None, "string", 200)
        }
        CommandResponse::Integer(v) => (true, Some(v.to_string()), None, "integer", 200),
        CommandResponse::Null => (true, None, None, "null", 200),
        CommandResponse::Error(e) => (false, None, Some(e), "error", 422),
        CommandResponse::Empty | CommandResponse::Close => {
            (false, None, Some("No response".into()), "error", 400)
        }
    };
    json_response(
        stream,
        status,
        ApiResponse {
            ok,
            response,
            error,
            kind,
        },
    )
    .await
}

async fn json_response(
    stream: &mut TcpStream,
    status: u16,
    body: ApiResponse,
) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(&body)
        .unwrap_or_else(|_| b"{\"ok\":false,\"error\":\"response encoding failed\"}".to_vec());
    respond(stream, status, "application/json", &bytes).await
}

async fn respond(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        413 => "Payload Too Large",
        _ => "Unprocessable Content",
    };
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nCache-Control: no-store\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.shutdown().await
}

#[cfg(test)]
mod tests {
    use super::tokenize;
    #[test]
    fn tokenizes_quoted_values_and_rejects_unclosed_quotes() {
        assert_eq!(
            tokenize("SET greeting \"hello there\""),
            Some(vec!["SET".into(), "greeting".into(), "hello there".into()])
        );
        assert_eq!(tokenize("GET 'unfinished"), None);
    }
}
