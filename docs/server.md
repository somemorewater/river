# TCP Server

River's networking layer lives in `src/server/tcp.rs`.

## Responsibilities

- Bind a TCP listener to `127.0.0.1:6379`
- Accept client connections continuously
- Spawn one Tokio task per client
- Read socket input line by line
- Send command responses back over the socket
- Handle disconnects and socket errors without panicking

## Shared State

All clients share the same store:

```
Arc<tokio::sync::Mutex<RiverStore>>
```

This keeps the first networked implementation simple and correct. The mutex ensures only one command mutates or reads the store at a time.

## Request Flow

```
client socket
  ↓
BufReader::lines()
  ↓
commands::handle_input()
  ↓
RiverStore
  ↓
write response
```

The TCP layer does not parse command syntax itself. It delegates parsing, validation, execution, and response formatting to the `commands` module.

## Manual Testing

Start River:

```bash
cargo run
```

Use a different port when `6379` is already occupied:

```bash
RIVER_ADDR=127.0.0.1:6380 cargo run
```

Connect with:

```bash
nc 127.0.0.1 6379
```

Then type commands:

```text
PING
SET name Water
GET name
STATS
HEALTH
```
