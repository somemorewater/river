# River Overview

River is a small, experimental in-memory key-value data store written in Rust.
The current implementation focuses on clarity and modularity: a Tokio TCP server reads client bytes, a RESP-inspired protocol layer decodes frames, a parser turns command parts into structured commands, and a store engine executes those commands against an in-memory `HashMap`.

## Goals

- Learn systems programming concepts in Rust
- Explore database internals incrementally (parsing → storage → networking → persistence → concurrency)
- Keep the codebase beginner-friendly and easy to extend

## Non-Goals (for now)

- No append-only log or advanced crash recovery yet
- No advanced concurrency optimizations yet
- No advanced query language; only a small command set

## High-Level Architecture

```
TCP client
  ↓
server::tcp (byte networking)
  ↓
protocol::resp (decode frame)
  ↓
commands::parser (parse command parts)
  ↓
commands::handle_parts (execute + format)
  ↓
store::engine::RiverStore (HashMap)
  ├── data
  ├── expirations
  └── cleanup
  ↓
persistence::storage (save mutations + cleanup)
  ↓
protocol::resp (encode response)
  ↓
socket response bytes
```

## Data Flow

1. The TCP server accepts a client connection on `127.0.0.1:6379`.
2. Each connected client is handled in its own Tokio task.
3. Socket bytes are decoded into RESP-style frames.
4. Command arrays are parsed into a `Command` enum (or rejected with an error).
5. The command module executes the command against shared `RiverStore` state.
6. Mutating commands save the store to disk.
7. A response is encoded as RESP and written back to the client.

## Why This Structure?

River is intentionally split into layers:

- **Input/Transport layer**: TCP networking lives in `server/`.
- **Protocol layer**: decodes and encodes RESP-style frames.
- **Parsing layer**: transforms command parts into structured commands.
- **Command layer**: executes parsed commands and formats responses.
- **Storage layer**: executes operations, tracks TTL metadata, and removes expired keys.
- **Persistence layer**: saves and restores database snapshots.
- **Observability layer**: the store tracks lightweight metrics such as key count and operation count.

This separation makes it easier to evolve the project without rewriting everything when more protocols are added.
