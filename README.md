# River — A Lightweight In-Memory Data Store in Rust

River is a small, experimental in-memory key-value database written in Rust.
It exists primarily as a learning project for systems programming and for exploring database internals (parsing, storage engines, networking, persistence, and concurrency).

River is inspired by Redis, but it is intentionally minimal and not intended to be a Redis replacement.

## Features

- In-memory key-value store (`HashMap<String, String>`)
- Tokio-based TCP server on `127.0.0.1:6379`
- Shared in-memory state across multiple clients
- Supported commands: `SET`, `GET`, `DEL`, `PING`, `STATS`, `HEALTH`, `EXIT`, `QUIT`
- Built-in runtime stats for keys and operations
- Rust-based performance and safety
- Modular architecture with networking isolated in `server/`

## Architecture Overview

Current data flow:

```
TCP Client
  ↓
TCP Layer
  ↓
Input Cleaning + Parsing
  ↓
Command Execution
  ↓
RiverStore (HashMap)
  ↓
Response (socket)
```

The same command path can be reused by future transports:

```
TCP / HTTP / CLI / RESP
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

Connect with `nc` or `telnet` from another terminal:

```bash
nc 127.0.0.1 6379
```

## Usage Examples

```
PING
PONG

SET name Water
OK

GET name
Water

DEL name
OK

GET name
NULL

STATS
keys: 0
operations: 4

HEALTH
status: OK
keys: 0
operations: 4
uptime: 0
```

### Input Rules (Strict)

- Leading/trailing whitespace is trimmed
- Empty lines are ignored (no output)
- Multiple spaces are normalized (whitespace is treated as separators)
- Invalid commands: `ERROR: Unknown command`
- Wrong argument count: `ERROR: Invalid syntax`

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
docs/
  overview.md
  architecture.md
  commands.md
  parser.md
  server.md
  store.md
  future.md
```

## Roadmap

Planned next steps:

- TCP server: accept client connections and reuse the same parser + command layer
- INFO command: richer runtime introspection (memory hints, persistence state)
- RESP protocol support
- Persistence: write snapshots / logs (planned via `serde` + `bincode`)
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
