# River Overview

River is a small, experimental in-memory key-value data store written in Rust.
The current implementation focuses on clarity and modularity: a Tokio TCP server reads client bytes, a RESP-inspired protocol layer decodes frames, a parser turns command parts into structured commands, and a store engine executes those commands against an in-memory `HashMap`.

## Goals

- Learn systems programming concepts in Rust
- Explore database internals incrementally (parsing → storage → networking → persistence → concurrency)
- Keep the codebase beginner-friendly and easy to extend

## Non-Goals (for now)

- No append-only log or advanced crash recovery yet
- No lock-free or transaction-grade concurrency yet
- No advanced query language; only a small command set

## High-Level Architecture

```
TCP client → server::tcp → protocol::resp ─┐
                                           ├→ commands::parser → handle_parts
Browser → server::web (HTTP/JSON) ─────────┘
                                                   ↓
  ↓
store::shared::ConcurrentStore (sharded, async RwLock)
  ├── partitions (shards)
  ├── per-shard expirations
  └── background cleanup
       persistence::storage (save mutations + cleanup)
TCP: RESP encoded socket response  |  HTTP: JSON response
```

## Data Flow

1. TCP accepts connections on `127.0.0.1:2007` (`RIVER_ADDR`); HTTP serves the embedded console on `127.0.0.1:8080` (`RIVER_HTTP_ADDR`).
2. TCP bytes are decoded as RESP; the HTTP endpoint decodes JSON and tokenizes its command string.
3. Both transports pass command parts to the shared parser and command handler.
4. The handler operates on shared `ConcurrentStore` state; mutations save a snapshot.
5. TCP responses are encoded as RESP and HTTP responses as JSON.

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
