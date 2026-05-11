# Parser

River uses a small, explicit parser to keep command handling safe and easy to extend.

Source:
- `src/commands/parser.rs`

## Purpose

The parser’s job is to convert raw user input into a structured representation:

- Input: command parts from the protocol layer, or a raw string for tests/local tooling
- Output: `Option<Command>` or a `ParseError`

This makes the storage engine independent of any particular input transport.

## Parsing Steps

1. **Empty handling**: if the command parts are empty, return `Ok(None)`.
2. **Command match**: uppercase the first part and match it against supported commands.
3. **Argument validation**: enforce exact argument counts:
   - `SET` requires 2 args (`key value`)
   - `GET` requires 1 arg (`key`)
   - `DEL` requires 1 arg (`key`)
   - `PING` requires 0 args
   - `STATS`/`/STATS` require 0 args
   - `HEALTH`/`/HEALTH` require 0 args
   - `EXIT`/`QUIT` require 0 args

## Output Types

### `Command`

The parser returns a `Command` enum to represent work to be done, for example:

- `Command::Set { key, value }`
- `Command::Get { key }`
- `Command::Ping`
- `Command::Stats`
- `Command::Health`

### `ParseError`

The parser can return:

- `UnknownCommand`: the command name is not recognized
- `InvalidSyntax`: wrong number of arguments for a recognized command

The command handler decides how to print these errors.

## Transport Reuse

Because the parser can operate on already-tokenized command parts, it is independent of TCP and whitespace. Future transports can reuse it.

Current TCP flow:

```
read RESP frame from socket
  ↓
frame_to_parts(frame)
  ↓
parse_parts(&parts)
  ↓
dispatch Command to RiverStore
  ↓
write response
```

Transport changes; parsing and storage do not.
