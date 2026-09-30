pub mod parser;

use crate::auth::AuthConfig;
use crate::metrics::Metrics;
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
    auth: &AuthConfig,
) -> CommandResponse {
    let started = std::time::Instant::now();
    let command = match parse_parts(parts) {
        Ok(Some(command)) => command,
        Ok(None) => return CommandResponse::Empty,
        Err(ParseError::UnknownCommand) => {
            store.metrics().record_command_error();
            tracing::warn!("rejected unknown command");
            return CommandResponse::Error("ERROR unknown command".to_string());
        }
        Err(ParseError::InvalidSyntax) => {
            store.metrics().record_command_error();
            tracing::warn!("rejected invalid syntax");
            return CommandResponse::Error("ERROR invalid syntax".to_string());
        }
    };

    let name = command.name();
    store.metrics().record_command(name);
    let response = execute_command(command, store, db_path, auth).await;
    tracing::debug!(
        command = name,
        elapsed_us = started.elapsed().as_micros(),
        "executed"
    );
    response
}

async fn execute_command(
    command: Command,
    store: &ConcurrentStore,
    db_path: Option<impl AsRef<Path>>,
    auth: &AuthConfig,
) -> CommandResponse {
    match command {
        Command::Set { key, value } => {
            store.set(key, value).await;
            if let Some(path) = db_path
                && persist_snapshot(store, path).await.is_err()
            {
                return CommandResponse::Error("ERROR persistence failed".to_string());
            }
            CommandResponse::Simple("OK".to_string())
        }
        Command::SetEx {
            key,
            seconds,
            value,
        } => {
            store.set_with_expiration(key, value, seconds).await;
            if let Some(path) = db_path
                && persist_snapshot(store, path).await.is_err()
            {
                return CommandResponse::Error("ERROR persistence failed".to_string());
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
            if let Some(path) = db_path
                && persist_snapshot(store, path).await.is_err()
            {
                return CommandResponse::Error("ERROR persistence failed".to_string());
            }
            CommandResponse::Simple("OK".to_string())
        }
        Command::Expire { key, seconds } => {
            let updated = store.expire(&key, seconds).await;
            if updated {
                if let Some(path) = db_path
                    && persist_snapshot(store, path).await.is_err()
                {
                    return CommandResponse::Error("ERROR persistence failed".to_string());
                }
                CommandResponse::Integer(1)
            } else {
                CommandResponse::Integer(0)
            }
        }
        Command::Ping => CommandResponse::Simple("PONG".to_string()),
        Command::Auth { password } => {
            // The secret never appears in logs, errors, or metrics: only the
            // outcome is recorded.
            if !auth.is_enabled() {
                store.metrics().record_auth_failure();
                return CommandResponse::Error(
                    "ERROR authentication not required".to_string(),
                );
            }
            if auth.verify(&password) {
                store.metrics().record_auth_success();
                CommandResponse::Simple("OK".to_string())
            } else {
                store.metrics().record_auth_failure();
                tracing::warn!("authentication failed");
                CommandResponse::Error("ERROR authentication failed".to_string())
            }
        }
        Command::Stats => {
            let stats = store.stats().await;
            let m = store.metrics();
            CommandResponse::Bulk(format!(
                "keys: {}\noperations: {}\nuptime: {}\ncommands: {}\ncommands_set: {}\ncommands_get: {}\ncommands_del: {}\ncommands_expire: {}\ncommands_setex: {}\ncommands_ping: {}\ncommands_stats: {}\ncommands_health: {}\ncommands_exit: {}\ncommands_auth: {}\nconnections_active: {}\nconnections_total: {}\ncommand_errors: {}\nprotocol_errors: {}\nauth_success: {}\nauth_failures: {}\nauth_required: {}\nexpired_keys: {}\ncleanup_runs: {}\npersistence_saves: {}\npersistence_failures: {}",
                stats.keys,
                stats.operations,
                store.uptime_seconds(),
                Metrics::load(&m.commands_total),
                Metrics::load(&m.cmd_set),
                Metrics::load(&m.cmd_get),
                Metrics::load(&m.cmd_del),
                Metrics::load(&m.cmd_expire),
                Metrics::load(&m.cmd_setex),
                Metrics::load(&m.cmd_ping),
                Metrics::load(&m.cmd_stats),
                Metrics::load(&m.cmd_health),
                Metrics::load(&m.cmd_exit),
                Metrics::load(&m.cmd_auth),
                Metrics::load(&m.connections_active),
                Metrics::load(&m.connections_total),
                Metrics::load(&m.command_errors),
                Metrics::load(&m.protocol_errors),
                Metrics::load(&m.auth_success),
                Metrics::load(&m.auth_failures),
                Metrics::load(&m.auth_required),
                Metrics::load(&m.expired_keys),
                Metrics::load(&m.cleanup_runs),
                Metrics::load(&m.persistence_saves),
                Metrics::load(&m.persistence_failures),
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
    match storage::save_to_disk(&snapshot, path) {
        Ok(()) => {
            store.metrics().record_persistence_saved();
            Ok(())
        }
        Err(error) => {
            store.metrics().record_persistence_failed();
            store.metrics().record_failed_execution();
            tracing::error!("persistence save failed: {error}");
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandResponse, handle_parts};
    use crate::auth::AuthConfig;
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
        let response =
            handle_parts(&["STATS".to_string()], &store, None::<&std::path::Path>, &AuthConfig::disabled()).await;
        let CommandResponse::Bulk(body) = response else {
            panic!("expected bulk response");
        };
        // First lines keep the historical shape; the rest is operational detail.
        assert!(body.starts_with("keys: 0\noperations: 0\n"), "{body}");
        for field in [
            "uptime:",
            "commands:",
            "commands_auth:",
            "connections_active:",
            "connections_total:",
            "command_errors:",
            "protocol_errors:",
            "auth_success:",
            "auth_failures:",
            "auth_required:",
            "expired_keys:",
            "persistence_saves:",
        ] {
            assert!(body.contains(field), "missing {field} in {body:?}");
        }
    }

    #[tokio::test]
    async fn auth_flows() {
        use crate::metrics::Metrics;
        let auth = AuthConfig::with_password("s3cr3t");

        // Wrong password: generic failure, session unaffected.
        let store = store();
        assert_eq!(
            handle_parts(
                &["AUTH".to_string(), "wrong".to_string()],
                &store,
                None::<&std::path::Path>,
                &auth,
            )
            .await,
            CommandResponse::Error("ERROR authentication failed".to_string())
        );
        assert_eq!(Metrics::load(&store.metrics().auth_failures), 1);
        assert_eq!(Metrics::load(&store.metrics().auth_success), 0);

        // Correct password.
        assert_eq!(
            handle_parts(
                &["AUTH".to_string(), "s3cr3t".to_string()],
                &store,
                None::<&std::path::Path>,
                &auth,
            )
            .await,
            CommandResponse::Simple("OK".to_string())
        );
        assert_eq!(Metrics::load(&store.metrics().auth_success), 1);

        // Disabled server: AUTH is rejected, never accepted.
        let open_store = ConcurrentStore::new(8);
        assert_eq!(
            handle_parts(
                &["AUTH".to_string(), "anything".to_string()],
                &open_store,
                None::<&std::path::Path>,
                &AuthConfig::disabled(),
            )
            .await,
            CommandResponse::Error("ERROR authentication not required".to_string())
        );
        // Failure message reveals nothing about the secret.
        let CommandResponse::Error(msg) = handle_parts(
            &["AUTH".to_string(), "s3cr3t".to_string()],
            &store,
            None::<&std::path::Path>,
            &AuthConfig::with_password("other"),
        )
        .await
        else {
            panic!("expected error");
        };
        assert!(!msg.contains("s3cr3t") && !msg.contains("other"), "{msg}");
    }

    #[tokio::test]
    async fn command_and_error_counters_increment() {        use crate::metrics::Metrics;
        let store = store();
        let m = store.metrics();

        handle_parts(
            &["SET".to_string(), "k".to_string(), "v".to_string()],
            &store,
            None::<&std::path::Path>,
                &AuthConfig::disabled(),
        )
        .await;
        handle_parts(&["GET".to_string(), "k".to_string()], &store, None::<&std::path::Path>, &AuthConfig::disabled()).await;
        handle_parts(&["NOPE".to_string()], &store, None::<&std::path::Path>, &AuthConfig::disabled()).await;

        assert_eq!(Metrics::load(&m.commands_total), 3);
        assert_eq!(Metrics::load(&m.cmd_set), 1);
        assert_eq!(Metrics::load(&m.cmd_get), 1);
        assert_eq!(Metrics::load(&m.command_errors), 1);
    }

    #[tokio::test]
    async fn formats_health_response() {
        let store = store();
        let response =
            handle_parts(&["HEALTH".to_string()], &store, None::<&std::path::Path>, &AuthConfig::disabled()).await;
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
                None::<&std::path::Path>,
                &AuthConfig::disabled(),
            )
            .await,
            CommandResponse::Error("ERROR invalid syntax".to_string())
        );
        assert_eq!(
            handle_parts(&["NOPE".to_string()], &store, None::<&std::path::Path>, &AuthConfig::disabled()).await,
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
                        &AuthConfig::disabled(),
            )
            .await,
            CommandResponse::Simple("OK".to_string())
        );

        assert_eq!(
            handle_parts(
                &["GET".to_string(), "name".to_string()],
                &store,
                None::<&std::path::Path>,
                        &AuthConfig::disabled(),
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
                        &AuthConfig::disabled(),
            )
            .await,
            CommandResponse::Integer(0)
        );

        assert_eq!(
            handle_parts(
                &["SET".to_string(), "session".to_string(), "abc".to_string()],
                &store,
                None::<&std::path::Path>,
                        &AuthConfig::disabled(),
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
                        &AuthConfig::disabled(),
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
                        &AuthConfig::disabled(),
            )
            .await,
            CommandResponse::Simple("OK".to_string())
        );

        assert_eq!(
            handle_parts(
                &["GET".to_string(), "session".to_string()],
                &store,
                None::<&std::path::Path>,
                        &AuthConfig::disabled(),
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
                Some(&path),
                &AuthConfig::disabled(),
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
