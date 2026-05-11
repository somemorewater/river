pub mod parser;

use crate::persistence::storage;
use crate::store::engine::RiverStore;
use parser::{Command, ParseError, parse_parts};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandResponse {
    Simple(String),
    Bulk(String),
    Null,
    Error(String),
    Empty,
    Close,
}

pub fn handle_parts(
    parts: &[String],
    store: &mut RiverStore,
    db_path: Option<impl AsRef<Path>>,
) -> CommandResponse {
    let command = match parse_parts(parts) {
        Ok(Some(command)) => command,
        Ok(None) => return CommandResponse::Empty,
        Err(ParseError::UnknownCommand) => {
            return CommandResponse::Error("ERROR unknown command".to_string());
        }
        Err(ParseError::InvalidSyntax) => {
            return CommandResponse::Error("ERROR invalid syntax".to_string());
        }
    };

    execute_command(command, store, db_path)
}

fn execute_command(
    command: Command,
    store: &mut RiverStore,
    db_path: Option<impl AsRef<Path>>,
) -> CommandResponse {
    match command {
        Command::Set { key, value } => {
            store.set(key, value);
            if let Some(path) = db_path {
                if let Err(error) = storage::save_to_disk(store, path) {
                    eprintln!("[ERROR] Failed to persist database: {error}");
                    return CommandResponse::Error("ERROR persistence failed".to_string());
                }
            }
            CommandResponse::Simple("OK".to_string())
        }
        Command::Get { key } => {
            let Some(value) = store.get(&key) else {
                return CommandResponse::Null;
            };

            CommandResponse::Bulk(value.to_string())
        }
        Command::Del { key } => {
            store.delete(&key);
            if let Some(path) = db_path {
                if let Err(error) = storage::save_to_disk(store, path) {
                    eprintln!("[ERROR] Failed to persist database: {error}");
                    return CommandResponse::Error("ERROR persistence failed".to_string());
                }
            }
            CommandResponse::Simple("OK".to_string())
        }
        Command::Ping => CommandResponse::Simple("PONG".to_string()),
        Command::Stats => {
            let stats = store.stats();
            CommandResponse::Bulk(format!(
                "keys: {}\noperations: {}",
                stats.keys, stats.operations
            ))
        }
        Command::Health => {
            let health = store.health();
            CommandResponse::Bulk(format!(
                "status: {}\nkeys: {}\noperations: {}\nuptime: {}",
                health.status, health.keys, health.operations, health.uptime
            ))
        }
        Command::Exit => CommandResponse::Close,
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandResponse, handle_parts};
    use crate::persistence::storage::load_from_disk;
    use crate::store::engine::RiverStore;
    use std::path::PathBuf;

    fn test_db_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("river-command-{name}-{}.db", std::process::id()))
    }

    #[test]
    fn formats_stats_response() {
        let mut store = RiverStore::new();
        assert_eq!(
            handle_parts(&["STATS".to_string()], &mut store, None::<&std::path::Path>),
            CommandResponse::Bulk("keys: 0\noperations: 0".to_string())
        );
    }

    #[test]
    fn formats_health_response() {
        let mut store = RiverStore::new();
        assert_eq!(
            handle_parts(
                &["HEALTH".to_string()],
                &mut store,
                None::<&std::path::Path>
            ),
            CommandResponse::Bulk("status: OK\nkeys: 0\noperations: 0\nuptime: 0".to_string())
        );
    }

    #[test]
    fn maps_parse_errors_to_messages() {
        let mut store = RiverStore::new();
        assert_eq!(
            handle_parts(
                &["HEALTH".to_string(), "now".to_string()],
                &mut store,
                None::<&std::path::Path>
            ),
            CommandResponse::Error("ERROR invalid syntax".to_string())
        );
        assert_eq!(
            handle_parts(&["NOPE".to_string()], &mut store, None::<&std::path::Path>),
            CommandResponse::Error("ERROR unknown command".to_string())
        );
    }

    #[test]
    fn handles_pre_tokenized_values_with_spaces() {
        let mut store = RiverStore::new();
        assert_eq!(
            handle_parts(
                &[
                    "SET".to_string(),
                    "name".to_string(),
                    "Water River".to_string()
                ],
                &mut store,
                None::<&std::path::Path>,
            ),
            CommandResponse::Simple("OK".to_string())
        );

        assert_eq!(
            handle_parts(
                &["GET".to_string(), "name".to_string()],
                &mut store,
                None::<&std::path::Path>,
            ),
            CommandResponse::Bulk("Water River".to_string())
        );
    }

    #[test]
    fn set_command_persists_store() {
        let path = test_db_path("set");
        let _ = std::fs::remove_file(&path);

        let mut store = RiverStore::new();
        assert_eq!(
            handle_parts(
                &["SET".to_string(), "name".to_string(), "Water".to_string()],
                &mut store,
                Some(&path)
            ),
            CommandResponse::Simple("OK".to_string())
        );

        let mut loaded = load_from_disk(&path)
            .expect("store should load")
            .expect("database file should exist");

        assert_eq!(loaded.get("name").map(String::as_str), Some("Water"));

        let _ = std::fs::remove_file(&path);
    }
}
