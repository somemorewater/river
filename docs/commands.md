# Commands and REPL

River currently provides an interactive terminal REPL that reads commands from `stdin` and prints responses to `stdout`.

Source:
- REPL + dispatch: `src/main.rs`
- Parsing: `src/commands/parser.rs`

## REPL Behavior

On startup, River prints a banner:

```
River DB started
```

Then it shows the prompt:

```
river >
```

The loop continues until:

- The user enters `EXIT` or `QUIT`, or
- `stdin` reaches EOF (Ctrl-D)

## Supported Commands

### `SET key value`

Stores `value` under `key`.

Response:
- `OK`

### `GET key`

Fetches the value for `key`.

Response:
- If present: prints the value
- If missing: prints `NULL`

### `DEL key`

Deletes `key` if it exists.

Response:
- `OK`

### `PING`

Health check / connectivity check (useful later for TCP).

Response:
- `PONG`

### `EXIT` / `QUIT`

Stops the program.

## Input Cleaning Rules

Before parsing, River normalizes input:

- Trims leading and trailing whitespace
- Treats runs of whitespace as separators (e.g. multiple spaces or tabs behave the same)
- Ignores empty / whitespace-only lines (no output and no error)

This makes command handling predictable and avoids subtle parsing bugs.

## Error Handling Strategy (Strict)

The parser returns one of:

- `Ok(None)` for empty input (silently ignored)
- `Ok(Some(Command))` for valid commands
- `Err(UnknownCommand)` → printed as `ERROR: Unknown command`
- `Err(InvalidSyntax)` → printed as `ERROR: Invalid syntax`

Examples:

- `HELLO` → `ERROR: Unknown command`
- `GET` (missing key) → `ERROR: Invalid syntax`
- `PING now` (extra arguments) → `ERROR: Invalid syntax`

The system is designed to never panic on user input.

