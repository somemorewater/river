# Commands

River currently accepts RESP-style arrays over TCP and writes RESP-style responses back to the socket.

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

Each RESP array sent by a client is parsed as one command. The connection remains open until:

- The user enters `EXIT` or `QUIT`, or
- The client disconnects

## Supported Commands

### `SET key value`

Stores `value` under `key`.

Response:
- `+OK`

### `SETEX key seconds value`

Stores `value` under `key` and sets an expiration in seconds.

Response:
- `+OK`

### `GET key`

Fetches the value for `key`.

Response:
- If present: bulk string, e.g. `$5\r\nWater\r\n`
- If missing: `$-1`

### `DEL key`

Deletes `key` if it exists.

Response:
- `+OK`

### `EXPIRE key seconds`

Sets an expiration on an existing key.

Response:
- `:1` if the TTL was attached
- `:0` if the key does not exist

### `PING`

Health check / connectivity check (useful later for TCP).

Response:
- `+PONG`

### `STATS` / `/STATS`

Prints runtime store metrics.

Response:

```text
$23
keys: 3
operations: 12
```

The operations count includes `SET`, `GET`, and `DEL` calls. Calling `STATS` does not increment the operations counter.

### `HEALTH` / `/HEALTH`

Prints a lightweight system health summary.

Response:

```text
$45
status: OK
keys: 3
operations: 12
uptime: 0
```

`uptime` is currently a placeholder field.

### `EXIT` / `QUIT`

Stops the program.

## RESP Command Shape

Commands are encoded as arrays of bulk strings:

```text
*2
$3
GET
$4
name
```

The protocol layer turns this into command parts: `["GET", "name"]`.

## Error Handling Strategy (Strict)

The parser returns one of:

- `Ok(None)` for empty input (silently ignored)
- `Ok(Some(Command))` for valid commands
- `Err(UnknownCommand)` → encoded as `-ERROR unknown command`
- `Err(InvalidSyntax)` → encoded as `-ERROR invalid syntax`

Examples:

- `HELLO` → `ERROR: Unknown command`
- `GET` (missing key) → `ERROR: Invalid syntax`
- `PING now` (extra arguments) → `ERROR: Invalid syntax`
- `EXPIRE key -1` → `ERROR: Invalid syntax`
- `SETEX key soon value` → `ERROR: Invalid syntax`
- `STATS extra` → `ERROR: Invalid syntax`
- `HEALTH extra` → `ERROR: Invalid syntax`

The system is designed to never panic on user input.
