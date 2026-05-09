# Commands

River currently accepts line-based commands over TCP and writes line-based responses back to the socket.

Source:
- Command execution + response formatting: `src/commands/mod.rs`
- Parsing: `src/commands/parser.rs`

## TCP Behavior

On startup, River binds to:

```
127.0.0.1:6379
```

Connect with:

```bash
nc 127.0.0.1 6379
```

Each line sent by a client is parsed as one command. The connection remains open until:

- The user enters `EXIT` or `QUIT`, or
- The client disconnects

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

### `STATS` / `/STATS`

Prints runtime store metrics.

Response:

```
keys: 3
operations: 12
```

The operations count includes `SET`, `GET`, and `DEL` calls. Calling `STATS` does not increment the operations counter.

### `HEALTH` / `/HEALTH`

Prints a lightweight system health summary.

Response:

```
status: OK
keys: 3
operations: 12
uptime: 0
```

`uptime` is currently a placeholder field.

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
- `STATS extra` → `ERROR: Invalid syntax`
- `HEALTH extra` → `ERROR: Invalid syntax`

The system is designed to never panic on user input.
