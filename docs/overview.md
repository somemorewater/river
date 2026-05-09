# River Overview

River is a small, experimental in-memory key-value data store written in Rust.
The current implementation focuses on clarity and modularity: a REPL reads user input, a parser turns input into structured commands, and a store engine executes those commands against an in-memory `HashMap`.

## Goals

- Learn systems programming concepts in Rust
- Explore database internals incrementally (parsing → storage → networking → persistence → concurrency)
- Keep the codebase beginner-friendly and easy to extend

## Non-Goals (for now)

- No networking (TCP server) yet
- No persistence (snapshots / AOF) yet
- No concurrency optimizations yet
- No advanced query language; only a small command set

## High-Level Architecture

```
stdin (user input)
  ↓
commands::parser (clean + parse)
  ↓
main.rs (dispatch / command handler)
  ↓
store::engine::RiverStore (HashMap)
  ↓
stdout (response)
```

## Data Flow

1. The REPL prints a prompt (`river >`) and waits for a line from `stdin`.
2. The line is cleaned and parsed into a `Command` enum (or rejected with an error).
3. `main.rs` dispatches the parsed `Command` to `RiverStore`.
4. A response is printed to `stdout` in a predictable format (`OK`, `NULL`, `PONG`, stats output, or `ERROR: ...`).

## Why This Structure?

River is intentionally split into layers:

- **Input/Transport layer**: today it's a CLI REPL; later it can be TCP.
- **Parsing layer**: transforms raw text into structured commands.
- **Storage layer**: executes operations against an engine (currently a `HashMap`).
- **Observability layer**: the store tracks lightweight metrics such as key count and operation count.

This separation makes it easier to evolve the project without rewriting everything when networking is added.
