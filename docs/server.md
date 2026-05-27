# TCP Server

River's networking layer lives in `src/server/tcp.rs`.

## Responsibilities

- Bind a TCP listener to `127.0.0.1:6379`
- Accept client connections continuously
- Spawn one Tokio task per client
- Read socket bytes into a buffer
- Pass bytes to the RESP protocol decoder
- Send RESP-encoded responses back over the socket
- Handle disconnects and socket errors without panicking

## Shared State

All clients share the same store:

```
Arc<ConcurrentStore>
```

`ConcurrentStore` is a sharded in-memory store (multiple partitions, each protected by an async `RwLock`). This allows concurrent reads and reduces contention under multi-client load while keeping write operations safe.

## Request Flow

```
client socket
  ↓
byte buffer
  ↓
protocol::resp::decode()
  ↓
commands::handle_parts()
  ↓
ConcurrentStore
  ↓
protocol::resp::encode()
  ↓
write response bytes
```

The TCP layer does not parse RESP syntax itself. It delegates framing to the `protocol` module and command interpretation to the `commands` module.

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

Then send RESP frames:

```text
*1
$4
PING
```

For a shell-friendly one-command check:

```bash
printf '*1\r\n$4\r\nPING\r\n' | nc 127.0.0.1 6379
```
