# Architecture

This document describes how River is organized today and why the modules are separated the way they are.

## System Flow

River currently behaves like a tiny Redis-style TCP database server:

```
TCP Client
  ↓
TCP Layer
  ↓
RESP Protocol Parser
  ↓
Command Parser
  ↓
Command Handler
  ↓
Store
  ├── Data Store
  ├── Expiration Store
  └── Cleanup System
  ↓
Persistence (mutations + cleanup)
  ↓
RESP Encoder
  ↓
Response Bytes
```

Mapping to code:

- **TCP Layer**: `src/server/tcp.rs`
- **Protocol Layer**: `src/protocol/resp.rs`, `src/protocol/frame.rs`
- **Parser**: `src/commands/parser.rs`
- **Command Handler**: `src/commands/mod.rs`
- **Store**: `src/store/engine.rs`
- **Persistence**: `src/persistence/storage.rs`
- **Bootstrap**: `src/main.rs`

## Why Modular Design?

River keeps parsing and storage separate for three reasons:

1. **Correctness and safety**: parsing and validation are centralized; the store never needs to interpret raw user input.
2. **Testability**: the parser and store can be tested independently.
3. **Future transport support**: HTTP, a CLI, or custom clients can reuse the same parser and dispatch logic.

## Parser vs. Storage Responsibilities

**Protocol responsibilities** (`protocol` module):

- Decode RESP-style frames from TCP bytes
- Encode server responses as RESP-style frames
- Represent protocol data with `Frame`
- Detect malformed or incomplete protocol messages

**Parser responsibilities** (`commands` module):

- Accept command parts from the protocol layer
- Validate command names and argument counts
- Return a structured `Command` value or a specific error

**Command responsibilities** (`commands` module):

- Convert parsed commands into store operations
- Format user-facing responses
- Map parser errors to stable error strings

**Storage responsibilities** (`store` module):

- Manage in-memory state (`HashMap<String, String>`)
- Implement `set`, `set_with_expiration`, `get`, `delete`, and `expire` operations
- Track key expiration metadata separately from stored values
- Remove expired keys during reads and background cleanup
- Track lightweight runtime metrics (`keys`, `operations`)
- Expose read-only stats through `store.stats()`

**Persistence responsibilities** (`persistence` module):

- Serialize `RiverStore` with `bincode`
- Write database snapshots to disk
- Load database snapshots on startup
- Keep file I/O separate from TCP networking

## TCP Networking

TCP is isolated in `src/server/tcp.rs`:

```
TCP connection
  ↓
read bytes
  ↓
protocol::resp::decode
  ↓
commands::parser::parse_parts
  ↓
commands::handle_parts
  ↓
RiverStore
  ↓
protocol::resp::encode
  ↓
write bytes
```

Each client connection runs in its own Tokio task. Shared database state is protected with `Arc<tokio::sync::Mutex<RiverStore>>`.

## Observability Path

River’s stats flow follows the same separation:

```
RESP Array (`STATS`)
  ↓
Parser (`Command::Stats`)
  ↓
Command Handler
  ↓
RiverStore::stats()
  ↓
keys / operations response
```

Metrics are intentionally owned by the store. The TCP layer does not calculate key counts or operation totals itself.

## Persistence Path

Persistence is triggered only by mutating commands:

```
SET / SETEX / DEL / EXPIRE
  ↓
RiverStore mutation
  ↓
persistence::storage::save_to_disk()
  ↓
river.db
```

Background expiration cleanup also saves when it removes stale keys. Read-only commands (`GET`, `PING`, `STATS`, `HEALTH`) do not normally write to disk.
