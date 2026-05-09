# Parser

River uses a small, explicit parser to keep command handling safe and easy to extend.

Source:
- `src/commands/parser.rs`

## Purpose

The parser’s job is to convert raw user input into a structured representation:

- Input: `&str` (a line from the REPL or a future TCP client)
- Output: `Option<Command>` or a `ParseError`

This makes the storage engine independent of any particular input transport.

## Parsing Steps

1. **Trim**: remove leading/trailing whitespace.
2. **Empty handling**: if the trimmed input is empty, return `Ok(None)` so the caller can silently ignore it.
3. **Tokenize**: split the line by whitespace using `split_whitespace()`.
   - This automatically normalizes multiple spaces/tabs into clean separation.
4. **Command match**: uppercase the first token and match it against supported commands.
5. **Argument validation**: enforce exact argument counts:
   - `SET` requires 2 args (`key value`)
   - `GET` requires 1 arg (`key`)
   - `DEL` requires 1 arg (`key`)
   - `PING` requires 0 args
   - `STATS`/`/STATS` require 0 args
   - `EXIT`/`QUIT` require 0 args

## Output Types

### `Command`

The parser returns a `Command` enum to represent work to be done, for example:

- `Command::Set { key, value }`
- `Command::Get { key }`
- `Command::Ping`
- `Command::Stats`

### `ParseError`

The parser can return:

- `UnknownCommand`: the command name is not recognized
- `InvalidSyntax`: wrong number of arguments for a recognized command

The REPL decides how to print these errors.

## Future TCP Integration

Because the parser operates on `&str` and returns a structured `Command`, it can be reused as-is when River adds networking.

The future TCP server’s flow will look like:

```
read line from socket
  ↓
parse_line(&line)
  ↓
dispatch Command to RiverStore
  ↓
write response
```

Transport changes; parsing and storage do not.
