# River — A Lightweight In-Memory Data Store in Rust

River is a small, experimental in-memory key-value database written in Rust.
It exists primarily as a learning project for systems programming and for exploring database internals (parsing, storage engines, networking, persistence, and concurrency).

River is inspired by Redis, but it is intentionally minimal and not intended to be a Redis replacement.

## Features

- In-memory key-value store (`HashMap<String, String>`)
- Tokio-based TCP server on `127.0.0.1:6379`
- RESP-inspired structured protocol
- TTL expiration with `EXPIRE` and `SETEX`
- Shared in-memory state across multiple clients
- Disk persistence with `serde` + `bincode`
- Supported commands: `SET`, `SETEX`, `GET`, `DEL`, `EXPIRE`, `PING`, `STATS`, `HEALTH`, `EXIT`, `QUIT`
- Built-in runtime stats for keys and operations
- Rust-based performance and safety
- Modular architecture with networking isolated in `server/` and protocol parsing in `protocol/`

## Architecture Overview

Current data flow:

```
TCP Client
  ↓
TCP Layer
  ↓
RESP Protocol Parser
  ↓
Command Parser
  ↓
Command Execution
  ↓
RiverStore (HashMap)
  ├── Data Store
  ├── Expiration Store
  └── Cleanup System
  ↓
Persistence Layer
  ↓
RESP Encoder
  ↓
Response (socket)
```

The same command path can be reused by future transports:

```
TCP / HTTP / CLI / custom clients
  ↓
Protocol Decoder
  ↓
Parser
  ↓
Store
  ↓
Response
```

## Installation

Prerequisites:
- Rust toolchain (stable) with Cargo

Build:

```bash
cargo build
```

## Running

Start the TCP server:

```bash
cargo run
```

You should see:

```
River DB server started on 127.0.0.1:6379
```

If port `6379` is already in use, run on another local port:

```bash
RIVER_ADDR=127.0.0.1:6380 cargo run
```

River persists data to `river.db` by default. To use another file:

```bash
RIVER_DB_PATH=/tmp/river-dev.db cargo run
```

Connect with `nc` or `telnet` from another terminal:

```bash
nc 127.0.0.1 6379
```

## Usage Examples

River now expects RESP-style frames over TCP.

```text
*1
$4
PING
```

Response:

```text
+PONG
```

Set and get a value:

```text
*3
$3
SET
$4
name
$5
Water
```

Response:

```text
+OK
```

```text
*2
$3
GET
$4
name
```

Response:

```text
$5
Water
```

Set a TTL on an existing key:

```text
*3
$6
EXPIRE
$4
name
$2
60
```

Response:

```text
:1
```

Set a value with a TTL in one command:

```text
*4
$5
SETEX
$7
session
$2
60
$3
abc
```

Response:

```text
+OK
```

Missing values return RESP null:

```text
$-1
```

Errors return RESP error frames:

```text
-ERROR unknown command
```

## Project Structure

```
src/
  main.rs            # Bootstrap/start TCP server
  store/
    mod.rs
    engine.rs        # RiverStore (HashMap-based data + metrics)
  commands/
    mod.rs           # Command execution + response formatting
    parser.rs        # Input cleaning + parsing into Command enum
  server/
    mod.rs
    tcp.rs           # Tokio TCP listener + client handling
  protocol/
    mod.rs
    frame.rs         # RESP-style frame definitions
    resp.rs          # RESP-style encoder/decoder
  persistence/
    mod.rs
    storage.rs       # bincode save/load helpers
docs/
  overview.md
  architecture.md
  commands.md
  expiration.md
  parser.md
  protocol.md
  persistence.md
  server.md
  store.md
  future.md
```

## Roadmap

Planned next steps:

- TCP server: accept client connections and reuse the same parser + command layer
- INFO command: richer runtime introspection (memory hints, persistence state)
- Protocol upgrades: pipelining, richer client compatibility
- Expiration upgrades: eviction policies, advanced cleanup scheduling
- Persistence upgrades: append-only logs, snapshots, crash recovery
- Benchmarking: measure throughput/latency (Criterion)
- Concurrency improvements: shared state, locking strategy, and command handling under load

## Philosophy

- River is not a production database and not a Redis replacement.
- River is a learning-focused systems project.
- The goal is to explore design tradeoffs (simplicity vs. performance, correctness vs. ergonomics) and evolve the system incrementally.

## Contributing

River is intentionally small and readable. If you want to contribute:

- Keep changes focused and well-scoped
- Prefer simple designs over cleverness
- Maintain separation between parsing and storage

Start by reading:
- `docs/overview.md`
- `docs/architecture.md`
