#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Set { key: String, value: String },
    Get { key: String },
    Del { key: String },
    Ping,
    Stats,
    Exit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    UnknownCommand,
    InvalidSyntax,
}

pub fn parse_line(input: &str) -> Result<Option<Command>, ParseError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let parts: Vec<&str> = trimmed.split_whitespace().collect();
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
                key: parts[1].to_string(),
                value: parts[2].to_string(),
            }))
        }
        "GET" => {
            if parts.len() != 2 {
                return Err(ParseError::InvalidSyntax);
            }
            Ok(Some(Command::Get {
                key: parts[1].to_string(),
            }))
        }
        "DEL" => {
            if parts.len() != 2 {
                return Err(ParseError::InvalidSyntax);
            }
            Ok(Some(Command::Del {
                key: parts[1].to_string(),
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
    use super::{Command, ParseError, parse_line};

    #[test]
    fn parses_stats_commands() {
        assert_eq!(parse_line("STATS"), Ok(Some(Command::Stats)));
        assert_eq!(parse_line("/stats"), Ok(Some(Command::Stats)));
    }

    #[test]
    fn rejects_stats_with_arguments() {
        assert_eq!(parse_line("STATS now"), Err(ParseError::InvalidSyntax));
    }

    #[test]
    fn ignores_empty_input() {
        assert_eq!(parse_line("   "), Ok(None));
    }

    #[test]
    fn normalizes_whitespace() {
        assert_eq!(
            parse_line("  SET   name   Water  "),
            Ok(Some(Command::Set {
                key: "name".to_string(),
                value: "Water".to_string()
            }))
        );
    }
}
