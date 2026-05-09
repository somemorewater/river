mod commands;
mod store;

use commands::parser::{Command, ParseError, parse_line};
use std::io::{self, Write};
use store::engine::RiverStore;

fn main() {
    let mut store = RiverStore::new();
    println!("River DB started");

    let stdin = io::stdin();
    let mut line = String::new();

    loop {
        print!("river > ");
        if io::stdout().flush().is_err() {
            break;
        }

        line.clear();
        match stdin.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }

        let command = match parse_line(&line) {
            Ok(Some(command)) => command,
            Ok(None) => continue,
            Err(ParseError::UnknownCommand) => {
                println!("ERROR: Unknown command");
                continue;
            }
            Err(ParseError::InvalidSyntax) => {
                println!("ERROR: Invalid syntax");
                continue;
            }
        };

        match command {
            Command::Set { key, value } => {
                store.set(key, value);
                println!("OK");
            }
            Command::Get { key } => match store.get(&key) {
                Some(value) => println!("{value}"),
                None => println!("NULL"),
            },
            Command::Del { key } => {
                store.delete(&key);
                println!("OK");
            }
            Command::Ping => println!("PONG"),
            Command::Exit => break,
        }
    }
}
