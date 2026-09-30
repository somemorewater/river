use super::client::{ClientError, RiverClient};
use super::{DEFAULT_HOST, DEFAULT_PORT};
use crate::protocol::frame::Frame;
use std::io::Write;
use tokio::io::{AsyncBufReadExt, BufReader};

pub struct CliArgs {
    pub host: String,
    pub port: u16,
    pub raw: bool,
}

impl CliArgs {
    pub fn default_local() -> Self {
        Self {
            host: DEFAULT_HOST.to_string(),
            port: DEFAULT_PORT,
            raw: false,
        }
    }

    /// Parse `cli` subcommand flags: `--host H --port P --raw`.
    /// Returns `Err(usage)` for bad flags; the caller prints it.
    pub fn parse(flags: &[String]) -> Result<Self, &'static str> {
        let mut args = Self::default_local();
        let mut i = 0;
        while i < flags.len() {
            match flags[i].as_str() {
                "--host" => {
                    i += 1;
                    args.host = flags.get(i).ok_or("Usage: cli [--host <host>] [--port <port>] [--raw]")?.clone();
                }
                "--port" => {
                    i += 1;
                    let port = flags.get(i).ok_or("Usage: cli [--host <host>] [--port <port>] [--raw]")?;
                    args.port = port.parse::<u16>().map_err(|_| "Invalid --port: expected 1-65535")?;
                }
                "--raw" => args.raw = true,
                _ => return Err("Usage: cli [--host <host>] [--port <port>] [--raw]"),
            }
            i += 1;
        }
        Ok(args)
    }

    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// What to do with one REPL input line. The server stays the source of truth
/// for command validity; this only handles local UX concerns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalAction {
    /// Send these parts as one RESP array command.
    Send(Vec<String>),
    /// Print a usage message instead of sending anything.
    Usage(&'static str),
    /// Print help, optionally for one command.
    Help(Option<String>),
    /// Quit the REPL without sending anything further.
    Quit,
    /// Empty input: just reprompt.
    Empty,
}

/// Split human input into command parts.
///
/// `SET` and `SETEX` join trailing words so values with spaces survive:
/// `SET message hello world` -> `["SET", "message", "hello world"]`.
/// Unknown commands pass through untouched so the server can reject them.
pub fn parse_line(line: &str) -> LocalAction {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.is_empty() {
        return LocalAction::Empty;
    }
    let name = tokens[0].to_ascii_uppercase();
    match name.as_str() {
        "SET" => {
            if tokens.len() < 3 {
                return LocalAction::Usage("Usage: SET <key> <value>");
            }
            LocalAction::Send(vec![
                "SET".to_string(),
                tokens[1].to_string(),
                tokens[2..].join(" "),
            ])
        }
        "SETEX" => {
            if tokens.len() < 4 {
                return LocalAction::Usage("Usage: SETEX <key> <seconds> <value>");
            }
            LocalAction::Send(vec![
                "SETEX".to_string(),
                tokens[1].to_string(),
                tokens[2].to_string(),
                tokens[3..].join(" "),
            ])
        }
        "GET" | "DEL" => {
            if tokens.len() != 2 {
                return LocalAction::Usage(if name == "GET" {
                    "Usage: GET <key>"
                } else {
                    "Usage: DEL <key>"
                });
            }
            LocalAction::Send(vec![name, tokens[1].to_string()])
        }
        "EXPIRE" => {
            if tokens.len() != 3 {
                return LocalAction::Usage("Usage: EXPIRE <key> <seconds>");
            }
            LocalAction::Send(vec![name, tokens[1].to_string(), tokens[2].to_string()])
        }
        "PING" | "STATS" | "HEALTH" => {
            if tokens.len() != 1 {
                return LocalAction::Usage(match name.as_str() {
                    "PING" => "Usage: PING",
                    "STATS" => "Usage: STATS",
                    _ => "Usage: HEALTH",
                });
            }
            LocalAction::Send(vec![name])
        }
        "HELP" => match tokens.len() {
            1 => LocalAction::Help(None),
            2 => LocalAction::Help(Some(tokens[1].to_ascii_uppercase())),
            _ => LocalAction::Usage("Usage: HELP [command]"),
        },
        "EXIT" | "QUIT" => {
            if tokens.len() != 1 {
                return LocalAction::Usage("Usage: QUIT");
            }
            LocalAction::Quit
        }
        _ => LocalAction::Send(tokens.iter().map(|t| t.to_string()).collect()),
    }
}

/// Render a server frame for humans. Raw RESP never reaches normal output.
pub fn format_response(frame: &Frame) -> String {
    match frame {
        Frame::Simple(text) | Frame::Bulk(text) => text.clone(),
        Frame::Integer(value) => value.to_string(),
        Frame::Null => "(nil)".to_string(),
        Frame::Error(message) => message.clone(),
        Frame::Array(items) => items
            .iter()
            .map(format_response)
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

/// Replace a trailing `uptime: <seconds>` line with a human duration.
/// Display-only; the server value is untouched.
pub fn prettify_uptime(body: &str) -> String {
    let mut lines: Vec<String> = body.lines().map(str::to_string).collect();
    if let Some(last) = lines.last_mut()
        && let Some(rest) = last.strip_prefix("uptime:")
    {
        let rest = rest.trim();
        if let Ok(secs) = rest.parse::<u64>() {
            *last = format!("uptime:     {}", format_duration(secs));
            // Align sibling `keys:`/`operations:` lines when present.
            for line in lines.iter_mut() {
                for key in ["status:", "keys:", "operations:"] {
                    if let Some(v) = line.strip_prefix(key) {
                        *line = format!("{key:12}{}", v.trim());
                        break;
                    }
                }
            }
        }
    }
    lines.join("\n")
}

fn format_duration(total: u64) -> String {
    const DAY: u64 = 86_400;
    const HOUR: u64 = 3_600;
    const MINUTE: u64 = 60;
    if total < MINUTE {
        return format!("{total}s");
    }
    if total < HOUR {
        return format!("{}m {}s", total / MINUTE, total % MINUTE);
    }
    if total < DAY {
        return format!("{}h {}m", total / HOUR, (total % HOUR) / MINUTE);
    }
    format!("{}d {}h", total / DAY, (total % DAY) / HOUR)
}

pub const HELP_TEXT: &str = "\
Commands:
  SET <key> <value>           Store a value (value may contain spaces)
  GET <key>                   Fetch a value ((nil) when missing)
  DEL <key>                   Delete a key
  EXPIRE <key> <seconds>      Attach a TTL (1 = set, 0 = missing)
  SETEX <key> <seconds> <value>  Store a value with a TTL
  PING                        Connectivity check
  STATS                       Key and operation counts
  HEALTH                      Status, counts, and uptime
  HELP [command]              This help, or help for one command
  QUIT                        Leave the CLI (EXIT works too)";

pub fn help_for(command: &str) -> Option<&'static str> {
    match command {
        "SET" => Some("Usage: SET <key> <value>\nStores <value> under <key>. Everything after the key is the value, so spaces are preserved."),
        "GET" => Some("Usage: GET <key>\nPrints the value, or (nil) when the key is missing or expired."),
        "DEL" => Some("Usage: DEL <key>\nDeletes the key if present. Always replies OK."),
        "EXPIRE" => Some("Usage: EXPIRE <key> <seconds>\nSets a TTL on an existing key. Replies 1 when set, 0 when the key is missing."),
        "SETEX" => Some("Usage: SETEX <key> <seconds> <value>\nStores <value> with a TTL in one step. Replies OK."),
        "PING" => Some("Usage: PING\nReplies PONG."),
        "STATS" => Some("Usage: STATS\nShows key and operation counts."),
        "HEALTH" => Some("Usage: HEALTH\nShows status, counts, and server uptime."),
        "HELP" => Some("Usage: HELP [command]\nLists commands, or details for one command."),
        "QUIT" | "EXIT" => Some("Usage: QUIT\nCloses the CLI. EXIT is an alias."),
        _ => None,
    }
}

/// Run the interactive session. Returns the process exit code.
pub async fn run_repl(args: &CliArgs) -> i32 {
    let mut client = match RiverClient::connect(&args.host, args.port, args.raw).await {
        Ok(client) => client,
        Err(ClientError::Connection(addr)) => {
            eprintln!("Could not connect to River at {addr}");
            eprintln!("Is the server running? Start it with `cargo run`.");
            return 1;
        }
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };

    println!("River CLI v{}", env!("CARGO_PKG_VERSION"));
    println!("Connected to {}", args.address());
    println!("\nType HELP for available commands.\n");

    let stdin = tokio::io::stdin();
    let mut lines = BufReader::new(stdin).lines();
    loop {
        print!("river> ");
        let _ = std::io::stdout().flush();
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("\nBye.");
                return 0;
            }
            next = lines.next_line() => {
                let line = match next {
                    Ok(Some(line)) => line,
                    Ok(None) => {
                        println!("Bye.");
                        return 0;
                    }
                    Err(e) => {
                        eprintln!("Input error: {e}");
                        return 1;
                    }
                };
                match parse_line(&line) {
                    LocalAction::Empty => {}
                    LocalAction::Quit => {
                        println!("Bye.");
                        return 0;
                    }
                    LocalAction::Usage(msg) => println!("{msg}"),
                    LocalAction::Help(topic) => match topic {
                        None => println!("{HELP_TEXT}"),
                        Some(cmd) => match help_for(&cmd) {
                            Some(text) => println!("{text}"),
                            None => println!("Unknown command '{cmd}'. Type HELP for the list."),
                        },
                    },
                    LocalAction::Send(parts) => {
                        let is_health = parts.first().is_some_and(|p| p == "HEALTH");
                        match client.execute(&parts).await {
                            Ok(frame) => {
                                let mut out = format_response(&frame);
                                if is_health && matches!(frame, Frame::Bulk(_)) {
                                    out = prettify_uptime(&out);
                                }
                                println!("{out}");
                            }
                            Err(ClientError::Closed) => {
                                println!("Connection closed by server.");
                                return 0;
                            }
                            Err(e) => println!("{e}"),
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HELP_TEXT, LocalAction, format_duration, format_response, help_for, parse_line, prettify_uptime};
    use crate::protocol::frame::Frame;

    #[test]
    fn empty_input_reprompts() {
        assert_eq!(parse_line(""), LocalAction::Empty);
        assert_eq!(parse_line("   "), LocalAction::Empty);
    }

    #[test]
    fn set_joins_values_with_spaces() {
        assert_eq!(
            parse_line("SET message hello world"),
            LocalAction::Send(vec![
                "SET".to_string(),
                "message".to_string(),
                "hello world".to_string(),
            ])
        );
        assert_eq!(
            parse_line("SETEX session 60 abc 123"),
            LocalAction::Send(vec![
                "SETEX".to_string(),
                "session".to_string(),
                "60".to_string(),
                "abc 123".to_string(),
            ])
        );
    }

    #[test]
    fn commands_are_uppercased() {
        assert_eq!(
            parse_line("get name"),
            LocalAction::Send(vec!["GET".to_string(), "name".to_string()])
        );
        assert_eq!(parse_line("quit"), LocalAction::Quit);
        assert_eq!(parse_line("exit"), LocalAction::Quit);
    }

    #[test]
    fn bad_arity_gives_usage_without_sending() {
        assert!(matches!(parse_line("SET"), LocalAction::Usage(_)));
        assert!(matches!(parse_line("SET k"), LocalAction::Usage(_)));
        assert!(matches!(parse_line("GET"), LocalAction::Usage(_)));
        assert!(matches!(parse_line("GET a b"), LocalAction::Usage(_)));
        assert!(matches!(parse_line("DEL"), LocalAction::Usage(_)));
        assert!(matches!(parse_line("EXPIRE k"), LocalAction::Usage(_)));
        assert!(matches!(parse_line("SETEX k 60"), LocalAction::Usage(_)));
        assert!(matches!(parse_line("PING now"), LocalAction::Usage(_)));
    }

    #[test]
    fn unknown_commands_pass_through_to_server() {
        assert_eq!(
            parse_line("FOO bar"),
            LocalAction::Send(vec!["FOO".to_string(), "bar".to_string()])
        );
    }

    #[test]
    fn formats_frames_without_resp() {
        assert_eq!(format_response(&Frame::Simple("PONG".into())), "PONG");
        assert_eq!(format_response(&Frame::Simple("OK".into())), "OK");
        assert_eq!(format_response(&Frame::Bulk("Water".into())), "Water");
        assert_eq!(format_response(&Frame::Null), "(nil)");
        assert_eq!(format_response(&Frame::Integer(1)), "1");
        assert_eq!(
            format_response(&Frame::Error("ERROR unknown command".into())),
            "ERROR unknown command"
        );
        let raw = format_response(&Frame::Bulk("a".into()));
        assert!(!raw.contains('$') && !raw.contains('+'));
    }

    #[test]
    fn help_lists_only_implemented_commands() {
        assert!(HELP_TEXT.contains("SET <key> <value>"));
        assert!(HELP_TEXT.contains("QUIT"));
        assert!(!HELP_TEXT.contains("DELETE"));
        assert!(!HELP_TEXT.contains("INFO"));
        assert!(help_for("NOPE").is_none());
        assert!(help_for("SET").unwrap().contains("Usage: SET"));
    }

    #[test]
    fn uptime_is_humanized() {
        assert_eq!(format_duration(45), "45s");
        assert_eq!(format_duration(134), "2m 14s");
        assert_eq!(format_duration(7_500), "2h 5m");
        let body = "status: OK\nkeys: 1\noperations: 3\nuptime: 134";
        let pretty = prettify_uptime(body);
        assert!(pretty.contains("2m 14s"), "{pretty}");
        assert!(!pretty.contains("uptime: 134"), "{pretty}");
    }
}
