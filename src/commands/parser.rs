#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Set { key: String, value: String },
    Get { key: String },
    Del { key: String },
    Ping,
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
        "EXIT" | "QUIT" => {
            if parts.len() != 1 {
                return Err(ParseError::InvalidSyntax);
            }
            Ok(Some(Command::Exit))
        }
        _ => Err(ParseError::UnknownCommand),
    }
}

