pub mod parser;

use crate::persistence::storage;
use crate::store::shared::ConcurrentStore;
use parser::{Command, ParseError, parse_parts};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandResponse {
    Simple(String),
    Integer(i64),
    Bulk(String),
    Null,
    Error(String),
    Empty,
    Close,
}

pub async fn handle_parts(
    parts: &[String],
    store: &ConcurrentStore,
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

    execute_command(command, store, db_path).await
}

async fn execute_command(
    command: Command,
    store: &ConcurrentStore,
    db_path: Option<impl AsRef<Path>>,
) -> CommandResponse {
    match command {
        Command::Set { key, value } => {
            store.set(key, value).await;
            if let Some(path) = db_path {
                if let Err(error) = persist_snapshot(store, path).await {
                    eprintln!("[ERROR] Failed to persist database: {error}");
                    return CommandResponse::Error("ERROR persistence failed".to_string());
                }
            }
            CommandResponse::Simple("OK".to_string())
        }
        Command::SetEx {
            key,
            seconds,
            value,
        } => {
            store.set_with_expiration(key, value, seconds).await;
            if let Some(path) = db_path {
                if let Err(error) = persist_snapshot(store, path).await {
                    eprintln!("[ERROR] Failed to persist database: {error}");
                    return CommandResponse::Error("ERROR persistence failed".to_string());
                }
            }
            CommandResponse::Simple("OK".to_string())
        }
        Command::Get { key } => {
            let Some(value) = store.get(&key).await else {
                return CommandResponse::Null;
            };

            CommandResponse::Bulk(value)
        }
        Command::Del { key } => {
            store.delete(&key).await;
            if let Some(path) = db_path {
                if let Err(error) = persist_snapshot(store, path).await {
                    eprintln!("[ERROR] Failed to persist database: {error}");
                    return CommandResponse::Error("ERROR persistence failed".to_string());
                }
            }
            CommandResponse::Simple("OK".to_string())
        }
        Command::Expire { key, seconds } => {
            let updated = store.expire(&key, seconds).await;
            if updated {
                if let Some(path) = db_path {
                    if let Err(error) = persist_snapshot(store, path).await {
                        eprintln!("[ERROR] Failed to persist database: {error}");
                        return CommandResponse::Error("ERROR persistence failed".to_string());
                    }
                }
                CommandResponse::Integer(1)
            } else {
                CommandResponse::Integer(0)
            }
        }
        Command::Ping => CommandResponse::Simple("PONG".to_string()),
        Command::Stats => {
            let stats = store.stats().await;
            CommandResponse::Bulk(format!(
                "keys: {}\noperations: {}",
                stats.keys, stats.operations
            ))
        }
        Command::Health => {
            let health = store.health().await;
            CommandResponse::Bulk(format!(
                "status: {}\nkeys: {}\noperations: {}\nuptime: {}",
                health.status, health.keys, health.operations, health.uptime_seconds
            ))
        }
        Command::Exit => CommandResponse::Close,
    }
}

async fn persist_snapshot(
    store: &ConcurrentStore,
    path: impl AsRef<Path>,
) -> Result<(), storage::PersistenceError> {
    let snapshot = store.snapshot().await;
    storage::save_to_disk(&snapshot, path)
}

#[cfg(test)]
mod tests {
    use super::{CommandResponse, handle_parts};
    use crate::persistence::storage::load_from_disk;
    use crate::store::shared::ConcurrentStore;
    use std::path::PathBuf;

    fn test_db_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("river-command-{name}-{}.db", std::process::id()))
    }

    fn store() -> ConcurrentStore {
        ConcurrentStore::new(8)
    }

    #[tokio::test]
    async fn formats_stats_response() {
        let store = store();
        assert_eq!(
            handle_parts(&["STATS".to_string()], &store, None::<&std::path::Path>).await,
            CommandResponse::Bulk("keys: 0\noperations: 0".to_string())
        );
    }

    #[tokio::test]
    async fn formats_health_response() {
        let store = store();
        let response =
            handle_parts(&["HEALTH".to_string()], &store, None::<&std::path::Path>).await;
        let CommandResponse::Bulk(body) = response else {
            panic!("expected bulk response");
        };
        assert!(body.starts_with("status: OK\nkeys: 0\noperations: 0\nuptime: "));
    }

    #[tokio::test]
    async fn maps_parse_errors_to_messages() {
        let store = store();
        assert_eq!(
            handle_parts(
                &["HEALTH".to_string(), "now".to_string()],
                &store,
                None::<&std::path::Path>
            )
            .await,
            CommandResponse::Error("ERROR invalid syntax".to_string())
        );
        assert_eq!(
            handle_parts(&["NOPE".to_string()], &store, None::<&std::path::Path>).await,
            CommandResponse::Error("ERROR unknown command".to_string())
        );
    }

    #[tokio::test]
    async fn handles_pre_tokenized_values_with_spaces() {
        let store = store();
        assert_eq!(
            handle_parts(
                &[
                    "SET".to_string(),
                    "name".to_string(),
                    "Water River".to_string()
                ],
                &store,
                None::<&std::path::Path>,
            )
            .await,
            CommandResponse::Simple("OK".to_string())
        );

        assert_eq!(
            handle_parts(
                &["GET".to_string(), "name".to_string()],
                &store,
                None::<&std::path::Path>,
            )
            .await,
            CommandResponse::Bulk("Water River".to_string())
        );
    }

    #[tokio::test]
    async fn expire_returns_integer_responses() {
        let store = store();

        assert_eq!(
            handle_parts(
                &[
                    "EXPIRE".to_string(),
                    "missing".to_string(),
                    "60".to_string()
                ],
                &store,
                None::<&std::path::Path>,
            )
            .await,
            CommandResponse::Integer(0)
        );

        assert_eq!(
            handle_parts(
                &["SET".to_string(), "session".to_string(), "abc".to_string()],
                &store,
                None::<&std::path::Path>,
            )
            .await,
            CommandResponse::Simple("OK".to_string())
        );

        assert_eq!(
            handle_parts(
                &[
                    "EXPIRE".to_string(),
                    "session".to_string(),
                    "60".to_string()
                ],
                &store,
                None::<&std::path::Path>,
            )
            .await,
            CommandResponse::Integer(1)
        );
    }

    #[tokio::test]
    async fn setex_sets_value_with_expiration() {
        let store = store();

        assert_eq!(
            handle_parts(
                &[
                    "SETEX".to_string(),
                    "session".to_string(),
                    "0".to_string(),
                    "abc".to_string(),
                ],
                &store,
                None::<&std::path::Path>,
            )
            .await,
            CommandResponse::Simple("OK".to_string())
        );

        assert_eq!(
            handle_parts(
                &["GET".to_string(), "session".to_string()],
                &store,
                None::<&std::path::Path>,
            )
            .await,
            CommandResponse::Null
        );
    }

    #[tokio::test]
    async fn set_command_persists_store() {
        let path = test_db_path("set");
        let _ = std::fs::remove_file(&path);

        let store = store();
        assert_eq!(
            handle_parts(
                &["SET".to_string(), "name".to_string(), "Water".to_string()],
                &store,
                Some(&path)
            )
            .await,
            CommandResponse::Simple("OK".to_string())
        );

        let mut loaded = load_from_disk(&path)
            .expect("store should load")
            .expect("database file should exist");

        assert_eq!(loaded.get("name").map(String::as_str), Some("Water"));

        let _ = std::fs::remove_file(&path);
    }
}
