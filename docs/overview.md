# River Overview

River is a small, experimental in-memory key-value data store written in Rust.
The current implementation focuses on clarity and modularity: a Tokio TCP server reads client input, a parser turns input into structured commands, and a store engine executes those commands against an in-memory `HashMap`.

## Goals

- Learn systems programming concepts in Rust
- Explore database internals incrementally (parsing → storage → networking → persistence → concurrency)
- Keep the codebase beginner-friendly and easy to extend

## Non-Goals (for now)

- No persistence (snapshots / AOF) yet
- No advanced concurrency optimizations yet
- No advanced query language; only a small command set

## High-Level Architecture

```
TCP client
  ↓
server::tcp (line-based networking)
  ↓
commands::parser (clean + parse)
  ↓
commands::handle_input (execute + format)
  ↓
store::engine::RiverStore (HashMap)
  ↓
socket response
```

## Data Flow

1. The TCP server accepts a client connection on `127.0.0.1:6379`.
2. Each connected client is handled in its own Tokio task.
3. A socket line is cleaned and parsed into a `Command` enum (or rejected with an error).
4. The command module executes the command against shared `RiverStore` state.
5. A response is written back to the client in a predictable format (`OK`, `NULL`, `PONG`, health/stats output, or `ERROR: ...`).

## Why This Structure?

River is intentionally split into layers:

- **Input/Transport layer**: TCP networking lives in `server/`.
- **Parsing layer**: transforms raw text into structured commands.
- **Command layer**: executes parsed commands and formats responses.
- **Storage layer**: executes operations against an engine (currently a `HashMap`).
- **Observability layer**: the store tracks lightweight metrics such as key count and operation count.

This separation makes it easier to evolve the project without rewriting everything when more protocols are added.
