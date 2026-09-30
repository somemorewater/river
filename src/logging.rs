use tracing::level_filters::LevelFilter;
use tracing_subscriber::fmt;

/// Environment variable selecting the server log level.
pub const LOG_ENV_VAR: &str = "RIVER_LOG";

/// Parse a log level name (case-insensitive).
/// Accepts `error`, `warn`, `info`, `debug`. Anything else is `None`.
pub fn parse_level(name: &str) -> Option<LevelFilter> {
    match name.to_ascii_lowercase().as_str() {
        "error" => Some(LevelFilter::ERROR),
        "warn" => Some(LevelFilter::WARN),
        "info" => Some(LevelFilter::INFO),
        "debug" => Some(LevelFilter::DEBUG),
        _ => None,
    }
}

/// Level from `RIVER_LOG`, falling back to `info` (with a stderr note) when
/// the variable is missing or invalid. Never panics, never exits.
pub fn level_from_env() -> LevelFilter {
    match std::env::var(LOG_ENV_VAR) {
        Ok(value) => match parse_level(&value) {
            Some(level) => level,
            None => {
                eprintln!(
                    "WARNING: invalid {LOG_ENV_VAR}={value:?}; falling back to info (expected error|warn|info|debug)"
                );
                LevelFilter::INFO
            }
        },
        Err(_) => LevelFilter::INFO,
    }
}

/// Install the process-wide tracing subscriber (human-readable, stderr).
/// Safe to call from both `server` and `cli` modes; user output stays on
/// stdout while logs go to stderr.
pub fn init_logging() {
    let level = level_from_env();
    let _ = fmt()
        .with_max_level(level)
        .with_target(false)
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::parse_level;
    use tracing::level_filters::LevelFilter;

    #[test]
    fn parses_supported_levels() {
        assert_eq!(parse_level("error"), Some(LevelFilter::ERROR));
        assert_eq!(parse_level("warn"), Some(LevelFilter::WARN));
        assert_eq!(parse_level("info"), Some(LevelFilter::INFO));
        assert_eq!(parse_level("debug"), Some(LevelFilter::DEBUG));
        assert_eq!(parse_level("DEBUG"), Some(LevelFilter::DEBUG));
    }

    #[test]
    fn rejects_unknown_levels() {
        assert_eq!(parse_level(""), None);
        assert_eq!(parse_level("trace"), None);
        assert_eq!(parse_level("verbose"), None);
    }
}
