pub mod parser;

use crate::store::engine::RiverStore;
use parser::{Command, ParseError, parse_line};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandResponse {
    Message(String),
    Empty,
    Close,
}

pub fn handle_input(input: &str, store: &mut RiverStore) -> CommandResponse {
    let command = match parse_line(input) {
        Ok(Some(command)) => command,
        Ok(None) => return CommandResponse::Empty,
        Err(ParseError::UnknownCommand) => {
            return CommandResponse::Message("ERROR: Unknown command".to_string());
        }
        Err(ParseError::InvalidSyntax) => {
            return CommandResponse::Message("ERROR: Invalid syntax".to_string());
        }
    };

    match command {
        Command::Set { key, value } => {
            store.set(key, value);
            CommandResponse::Message("OK".to_string())
        }
        Command::Get { key } => {
            let value = store
                .get(&key)
                .map_or_else(|| "NULL".to_string(), ToString::to_string);
            CommandResponse::Message(value)
        }
        Command::Del { key } => {
            store.delete(&key);
            CommandResponse::Message("OK".to_string())
        }
        Command::Ping => CommandResponse::Message("PONG".to_string()),
        Command::Stats => {
            let stats = store.stats();
            CommandResponse::Message(format!(
                "keys: {}\noperations: {}",
                stats.keys, stats.operations
            ))
        }
        Command::Health => {
            let health = store.health();
            CommandResponse::Message(format!(
                "status: {}\nkeys: {}\noperations: {}\nuptime: {}",
                health.status, health.keys, health.operations, health.uptime
            ))
        }
        Command::Exit => CommandResponse::Close,
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandResponse, handle_input};
    use crate::store::engine::RiverStore;

    #[test]
    fn formats_stats_response() {
        let mut store = RiverStore::new();
        assert_eq!(
            handle_input("STATS", &mut store),
            CommandResponse::Message("keys: 0\noperations: 0".to_string())
        );
    }

    #[test]
    fn formats_health_response() {
        let mut store = RiverStore::new();
        assert_eq!(
            handle_input("HEALTH", &mut store),
            CommandResponse::Message("status: OK\nkeys: 0\noperations: 0\nuptime: 0".to_string())
        );
    }

    #[test]
    fn maps_parse_errors_to_messages() {
        let mut store = RiverStore::new();
        assert_eq!(
            handle_input("HEALTH now", &mut store),
            CommandResponse::Message("ERROR: Invalid syntax".to_string())
        );
        assert_eq!(
            handle_input("NOPE", &mut store),
            CommandResponse::Message("ERROR: Unknown command".to_string())
        );
    }
}
