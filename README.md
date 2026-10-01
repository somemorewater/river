# River — A Lightweight In-Memory Data Store in Rust

River is a small, experimental in-memory key-value database written in Rust.
It exists primarily as a learning project for systems programming and for exploring database internals (parsing, storage engines, networking, persistence, and concurrency).

River is inspired by Redis, but it is intentionally minimal and not intended to be a Redis replacement.

## Features

- In-memory key-value store (`HashMap<String, String>`)
- Tokio-based TCP server on `127.0.0.1:2007`
- RESP-inspired structured protocol
- TTL expiration with `EXPIRE` and `SETEX`
- Shared in-memory state across multiple clients (sharded, async `RwLock`)
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
ConcurrentStore (sharded partitions)
  ├── Data Store (per shard)
  ├── Expiration Store (per shard)
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

River v1.0.0 ships one executable per platform with both subcommands:
`river server` starts the database, `river cli` opens the interactive client.

Download the archive for your platform from the
[v1.0.0 release](https://github.com/somemorewater/river/releases/tag/v1.0.0):

| Platform | Archive |
|---|---|
| Linux x86_64 | [river-v1.0.0-linux-x86_64.tar.gz](https://github.com/somemorewater/river/releases/download/v1.0.0/river-v1.0.0-linux-x86_64.tar.gz) |
| Linux aarch64 | [river-v1.0.0-linux-aarch64.tar.gz](https://github.com/somemorewater/river/releases/download/v1.0.0/river-v1.0.0-linux-aarch64.tar.gz) |
| macOS x86_64 | [river-v1.0.0-macos-x86_64.tar.gz](https://github.com/somemorewater/river/releases/download/v1.0.0/river-v1.0.0-macos-x86_64.tar.gz) |
| macOS aarch64 | [river-v1.0.0-macos-aarch64.tar.gz](https://github.com/somemorewater/river/releases/download/v1.0.0/river-v1.0.0-macos-aarch64.tar.gz) |
| Windows x86_64 | [river-v1.0.0-windows-x86_64.zip](https://github.com/somemorewater/river/releases/download/v1.0.0/river-v1.0.0-windows-x86_64.zip) |
| Windows aarch64 | [river-v1.0.0-windows-aarch64.zip](https://github.com/somemorewater/river/releases/download/v1.0.0/river-v1.0.0-windows-aarch64.zip) |

Verify checksums with
[SHA256SUMS](https://github.com/somemorewater/river/releases/download/v1.0.0/SHA256SUMS):

```bash
sha256sum -c SHA256SUMS
```

On Windows (PowerShell):

```powershell
(Get-FileHash river-v1.0.0-windows-x86_64.zip -Algorithm SHA256).Hash
# compare against the matching line in SHA256SUMS
```

Extract and run (Linux/macOS):

```bash
tar -xzf river-v1.0.0-linux-x86_64.tar.gz
cd linux-x86_64
./river server
```

On macOS, clear the quarantine flag on first run if Gatekeeper blocks it:

```bash
xattr -d com.apple.quarantine river
```

On Windows (PowerShell):

```powershell
Expand-Archive river-v1.0.0-windows-x86_64.zip -DestinationPath river-win
cd river-win
.\river.exe server
```

Then connect with the CLI from another terminal:

```bash
./river cli
```

```text
river> PING
PONG
```

Optional system-wide install (Linux/macOS):

```bash
sudo install -m 0755 river /usr/local/bin/river
```

### Build from source

Prerequisites:
- Rust toolchain (stable) with Cargo

```bash
cargo build --release
./target/release/river server
```

## Running

Start the TCP server (River is currently a TCP database server, not a CLI/REPL application):

```bash
cargo run
```

Tune concurrency (shard count):

```bash
RIVER_SHARDS=64 cargo run
```

You should see:

```
River DB server started on 127.0.0.1:2007
```

If port `2007` is already in use, run on another local port:

```bash
RIVER_ADDR=127.0.0.1:2008 cargo run
```

River persists data to `river.db` by default. To use another file:

```bash
RIVER_DB_PATH=/tmp/river-dev.db cargo run
```

Connect with `nc` or `telnet` from another terminal:

```bash
nc 127.0.0.1 2007
```

Or use the built-in interactive CLI (human-friendly, speaks RESP under the hood):

```bash
cargo run -- cli
```

```text
river> SET name Water
OK
river> GET name
Water
```

`cargo run -- cli --host 127.0.0.1 --port 2007` overrides the address;
`--raw` shows raw RESP frames for debugging. Type `HELP` inside the CLI.
With a password-protected server, set the same password for the CLI:

```bash
RIVER_PASSWORD=s3cr3t cargo run -- cli
```

See `docs/security.md`. Password auth does not encrypt traffic.

Log level via `RIVER_LOG=debug cargo run -- server` (default `info`).
`STATS` reports keys, operations, uptime, per-command counts, connections,
errors, expirations, and persistence counters — see `docs/observability.md`.

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
  lib.rs             # Library entry (shared by benches)
  main.rs            # Bootstrap/start TCP server
  store/
    mod.rs
    engine.rs        # RiverStore (HashMap-based data + metrics)
    shared.rs        # ConcurrentStore (sharded async access layer)
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

### IMPLEMENTED

- TCP server: accepts client connections and reuses the same parser + command layer
- RESP-based protocol with pipelining and partial-read handling
- TTL expiration (`EXPIRE`, `SETEX`) with passive reads, 1s background cleanup, startup cleanup, and persisted expiration metadata
- Snapshot persistence (`serde` + `bincode`, atomic temp-file + rename, restore on restart)
- Sharded `RwLock` concurrency (`ConcurrentStore`, configurable via `RIVER_SHARDS`)
- `STATS` / `HEALTH` with real key counts, operation counts, and real uptime
- Criterion concurrency benchmark (`cargo bench --bench store_concurrency`, parallel GET)

### PLANNED (not implemented)

- Crash recovery / durability: append-only log or equivalent, checksums/manifests, documented guarantees (snapshots only today)
- INFO command: richer runtime introspection (memory hints, persistence state)
- Richer Redis client compatibility
- HTTP endpoints for status or diagnostics
- Expiration upgrades: eviction policies (LRU/LFU), advanced cleanup scheduling, per-key TTL inspection
- Persistence upgrades: append-only logs, background snapshots without blocking, checksums, compression, crash recovery manifests
- Next scaling steps: dedicated worker model, richer observability

River does NOT currently implement: `DELETE` alias (only `DEL`), INFO, AOF, replication, transactions, Pub/Sub, lists, sets, or streams.

## Benchmarking

Run the store concurrency benchmark:

```bash
cargo bench --bench store_concurrency
```

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
