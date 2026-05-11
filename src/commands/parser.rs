#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Set { key: String, value: String },
    Get { key: String },
    Del { key: String },
    Ping,
    Stats,
    Health,
    Exit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    UnknownCommand,
    InvalidSyntax,
}

pub fn parse_parts(parts: &[String]) -> Result<Option<Command>, ParseError> {
    if parts.is_empty() {
        return Ok(None);
    }

    let command = parts[0].to_ascii_uppercase();
    match command.as_str() {
        "SET" => {
            if parts.len() != 3 {
                return Err(ParseError::InvalidSyntax);
            }
            Ok(Some(Command::Set {
                key: parts[1].clone(),
                value: parts[2].clone(),
            }))
        }
        "GET" => {
            if parts.len() != 2 {
                return Err(ParseError::InvalidSyntax);
            }
            Ok(Some(Command::Get {
                key: parts[1].clone(),
            }))
        }
        "DEL" => {
            if parts.len() != 2 {
                return Err(ParseError::InvalidSyntax);
            }
            Ok(Some(Command::Del {
                key: parts[1].clone(),
            }))
        }
        "PING" => {
            if parts.len() != 1 {
                return Err(ParseError::InvalidSyntax);
            }
            Ok(Some(Command::Ping))
        }
        "STATS" | "/STATS" => {
            if parts.len() != 1 {
                return Err(ParseError::InvalidSyntax);
            }
            Ok(Some(Command::Stats))
        }
        "HEALTH" | "/HEALTH" => {
            if parts.len() != 1 {
                return Err(ParseError::InvalidSyntax);
            }
            Ok(Some(Command::Health))
        }
        "EXIT" | "QUIT" => {
            if parts.len() != 1 {
                return Err(ParseError::InvalidSyntax);
            }
            Ok(Some(Command::Exit))
        }
        _ => Err(ParseError::UnknownCommand),
    }
}

#[cfg(test)]
mod tests {
    use super::{Command, ParseError, parse_parts};

    #[test]
    fn parses_stats_commands() {
        assert_eq!(
            parse_parts(&["STATS".to_string()]),
            Ok(Some(Command::Stats))
        );
        assert_eq!(
            parse_parts(&["/stats".to_string()]),
            Ok(Some(Command::Stats))
        );
    }

    #[test]
    fn rejects_stats_with_arguments() {
        assert_eq!(
            parse_parts(&["STATS".to_string(), "now".to_string()]),
            Err(ParseError::InvalidSyntax)
        );
    }

    #[test]
    fn parses_health_commands() {
        assert_eq!(
            parse_parts(&["HEALTH".to_string()]),
            Ok(Some(Command::Health))
        );
        assert_eq!(
            parse_parts(&["/health".to_string()]),
            Ok(Some(Command::Health))
        );
    }

    #[test]
    fn rejects_health_with_arguments() {
        assert_eq!(
            parse_parts(&["HEALTH".to_string(), "now".to_string()]),
            Err(ParseError::InvalidSyntax)
        );
    }

    #[test]
    fn ignores_empty_input() {
        assert_eq!(parse_parts(&[]), Ok(None));
    }

    #[test]
    fn parses_pre_tokenized_parts_without_splitting_values() {
        assert_eq!(
            parse_parts(&[
                "SET".to_string(),
                "name".to_string(),
                "Water River".to_string()
            ]),
            Ok(Some(Command::Set {
                key: "name".to_string(),
                value: "Water River".to_string()
            }))
        );
    }
}
