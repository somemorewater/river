pub mod client;
pub mod repl;

pub use client::RiverClient;
pub use repl::{CliArgs, format_response, run_repl};

/// Address the CLI connects to when no flags are given.
pub const DEFAULT_HOST: &str = "127.0.0.1";
pub const DEFAULT_PORT: u16 = 2007;
