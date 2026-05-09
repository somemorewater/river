# River — A Lightweight In-Memory Data Store in Rust

River is a small, experimental in-memory key-value database written in Rust.
It exists primarily as a learning project for systems programming and for exploring database internals (parsing, storage engines, networking, persistence, and concurrency).

River is inspired by Redis, but it is intentionally minimal and not intended to be a Redis replacement.

## Features

- In-memory key-value store (`HashMap<String, String>`)
- CLI-based REPL interface
- Supported commands: `SET`, `GET`, `DEL`, `PING`, `EXIT`, `QUIT`
- Rust-based performance and safety
- Modular architecture designed for future TCP integration

## Architecture Overview

Current data flow:

```
User Input (stdin)
  ↓
Input Cleaning + Parsing
  ↓
Command Handler (main loop)
  ↓
RiverStore (HashMap)
  ↓
Response (stdout)
```

Conceptually, the project is structured so the parser can be reused later:

```
TCP Client / CLI
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

Start the interactive REPL:

```bash
cargo run
```

You should see:

```
River DB started
river >
```

Exit with `EXIT`, `QUIT`, or EOF (Ctrl-D).

## Usage Examples

```
river > PING
PONG

river > SET name Water
OK

river > GET name
Water

river > DEL name
OK

river > GET name
NULL
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
  main.rs            # REPL + command dispatch
  store/
    mod.rs
    engine.rs        # RiverStore (HashMap-based)
  commands/
    mod.rs
    parser.rs        # Input cleaning + parsing into Command enum
docs/
  overview.md
  architecture.md
  commands.md
  parser.md
  store.md
  future.md
```

## Roadmap

Planned next steps:

- TCP server: accept client connections and reuse the same parser + command layer
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

